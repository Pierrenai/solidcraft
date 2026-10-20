import * as THREE from 'three';
import { Brush, Evaluator, ADDITION, SUBTRACTION, INTERSECTION } from 'three-bvh-csg';

const evaluator = new Evaluator();
evaluator.attributes = ['position', 'normal'];
evaluator.useGroups = false;
export const clone = value => structuredClone(value);
export const uid = () => crypto.randomUUID();
export const labels = { box: 'Bloc', cylinder: 'Cylindre', tube: 'Tube', sketch: 'Extrusion', boolean: 'Solide combiné' };
export function body(type, params = {}) {
  const defaults = {box:{width:60,depth:40,height:12},cylinder:{radius:15,height:30},tube:{radius:20,innerRadius:15,height:30},sketch:{height:12,points:[[-25,-15],[25,-15],[25,5],[5,5],[5,25],[-25,25]]}};
  return { id:uid(), name:labels[type], type, params:{...defaults[type],...params}, position:[0,0,0], holes:[], visible:true, color:'#e5aa65' };
}
function solidOp(a,b,operation) {
  const aa = new Brush(a), bb = new Brush(b);
  aa.updateMatrixWorld(); bb.updateMatrixWorld();
  const result = evaluator.evaluate(aa,bb,operation).geometry;
  if (!result.attributes.position?.count) throw new Error('Cette opération produit un solide vide.');
  result.computeVertexNormals();
  return result;
}
export function geometry(item) {
  const p=item.params;
  let g;
  if(item.type==='box') {
    g=new THREE.BoxGeometry(p.width,p.depth,p.height);
    g.translate(0,0,p.height/2);
  } else if(item.type==='cylinder') {
    g=new THREE.CylinderGeometry(p.radius,p.radius,p.height,96);
    g.rotateX(Math.PI/2); g.translate(0,0,p.height/2);
  } else if(item.type==='tube') {
    const shape=new THREE.Shape();
    shape.absarc(0,0,p.radius,0,Math.PI*2,false);
    const hole=new THREE.Path(); hole.absarc(0,0,p.innerRadius,0,Math.PI*2,true); shape.holes.push(hole);
    g=new THREE.ExtrudeGeometry(shape,{depth:p.height,bevelEnabled:false,curveSegments:64,steps:1});
  } else if(item.type==='sketch') {
    const shape=new THREE.Shape(p.points.map(([x,y])=>new THREE.Vector2(x,y)));
    shape.closePath();
    g=new THREE.ExtrudeGeometry(shape,{depth:p.height,bevelEnabled:false,steps:1});
  } else if(item.type==='boolean') {
    const a=worldGeometry(item.sources[0]), b=worldGeometry(item.sources[1]);
    try { g=solidOp(a,b,{union:ADDITION,subtract:SUBTRACTION,intersect:INTERSECTION}[item.operation]); }
    finally { a.dispose(); b.dispose(); }
  } else throw new Error('Type de solide inconnu.');
  for(const hole of item.holes) {
    g.computeBoundingBox();
    const bounds=g.boundingBox;
    const length=bounds.max.z-bounds.min.z+4;
    const cut=new THREE.CylinderGeometry(hole.diameter/2,hole.diameter/2,length,64);
    cut.rotateX(Math.PI/2); cut.translate(hole.x,hole.y,(bounds.max.z+bounds.min.z)/2);
    let next;
    try { next=solidOp(g,cut,SUBTRACTION); } finally {cut.dispose();}
    g.dispose(); g=next;
  }
  g.computeBoundingBox();
  return g;
}
export function worldGeometry(item) {
  const g=geometry(item);
  g.translate(...item.position);
  return g;
}
export function volume(g) {
  const p=g.attributes.position, ix=g.index;
  const count=ix?ix.count:p.count;
  let sum=0; const a=new THREE.Vector3(),b=new THREE.Vector3(),c=new THREE.Vector3();
  for(let i=0;i<count;i+=3) {
    a.fromBufferAttribute(p,ix?ix.getX(i):i); b.fromBufferAttribute(p,ix?ix.getX(i+1):i+1); c.fromBufferAttribute(p,ix?ix.getX(i+2):i+2);
    sum+=a.dot(b.cross(c))/6;
  }
  return Math.abs(sum);
}
export function validPolygon(points) {
  if(!Array.isArray(points)||points.length<3||points.length>100) return false;
  const cross=(a,b,c)=>(b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0]);
  let area=0;
  for(let i=0;i<points.length;i++) {
    const a=points[i], b=points[(i+1)%points.length];
    if(!Array.isArray(a)||a.length!==2||!a.every(v=>Number.isFinite(v)&&Math.abs(v)<=5000))return false;
    if(Math.hypot(a[0]-b[0],a[1]-b[1])<0.01)return false;
    area+=a[0]*b[1]-b[0]*a[1];
    for(let j=i+2;j<points.length;j++) {
      if(i===0&&j===points.length-1)continue;
      const c=points[j],d=points[(j+1)%points.length];
      if(cross(a,b,c)*cross(a,b,d)<=0&&cross(c,d,a)*cross(c,d,b)<=0) return false;
    }
  }
  return Math.abs(area)>0.01;
}
export function validateProject(value) {
  if(value?.format!=='solidcraft'||value.version!==1||typeof value.name!=='string'||value.name.length>100||!Array.isArray(value.bodies)||value.bodies.length>100) throw new Error('Projet SolidCraft invalide (version 1 attendue, 100 solides maximum).');
  let count=0;
  const check=(b,depth=0)=>{
    if(++count>300||depth>12)throw new Error('Projet trop complexe.');
    const finite=v=>typeof v==='number'&&Number.isFinite(v)&&Math.abs(v)<=5000;
    const positive=v=>finite(v)&&v>=0.1;
    if(!b||!Object.hasOwn(labels,b.type)||typeof b.name!=='string'||b.name.length>100||typeof b.id!=='string'||!/^[a-zA-Z0-9_-]{1,100}$/.test(b.id)||!Array.isArray(b.position)||b.position.length!==3||!b.position.every(finite)||!Array.isArray(b.holes)||b.holes.length>50||!/^#[0-9a-f]{6}$/i.test(b.color)||typeof b.visible!=='boolean')throw new Error('Données de solide invalides.');
    const p=b.params;
    if(!p||typeof p!=='object')throw new Error('Paramètres manquants.');
    if(b.type==='boolean') {
      if(!['union','subtract','intersect'].includes(b.operation)||!Array.isArray(b.sources)||b.sources.length!==2)throw new Error('Opération invalide.');
      b.sources.forEach(s=>check(s,depth+1));
    } else {
      if(!positive(p.height))throw new Error('Hauteur invalide.');
      if(b.type==='box'&&(!positive(p.width)||!positive(p.depth)))throw new Error('Dimensions invalides.');
      if(['tube','cylinder'].includes(b.type)&&!positive(p.radius))throw new Error('Rayon invalide.');
      if(b.type==='tube'&&(!positive(p.innerRadius)||p.innerRadius>=p.radius))throw new Error('Le rayon intérieur doit être inférieur au rayon extérieur.');
      if(b.type==='sketch'&&!validPolygon(p.points))throw new Error('Contour invalide ou croisé.');
    }
    if(!b.holes.every(h=>h&&positive(h.diameter)&&finite(h.x)&&finite(h.y)))throw new Error('Perçage invalide.');
  };
  value.bodies.forEach(b=>check(b));
  if(new Set(value.bodies.map(b=>b.id)).size!==value.bodies.length)throw new Error('Identifiants dupliqués.');
  return value;
}
export function demoProject() {
  const plate=body('box',{width:80,depth:50,height:8});
  plate.name='Platine de fixation';
  plate.holes=[{diameter:6,x:-30,y:-15},{diameter:6,x:30,y:-15},{diameter:6,x:30,y:15},{diameter:6,x:-30,y:15}];
  return {format:'solidcraft',version:1,name:'Platine de fixation',bodies:[plate]};
}
