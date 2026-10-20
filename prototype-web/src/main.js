import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { STLExporter } from 'three/addons/exporters/STLExporter.js';
import { body, clone, uid, labels, geometry, volume, validPolygon, validateProject, demoProject } from './model.js';
import './style.css';

const $ = s=>document.querySelector(s);
const esc = s=>String(s).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const icons={
 box:'M4 7 12 3l8 4v10l-8 4-8-4V7Zm0 0 8 4 8-4M12 11v10',
 cylinder:'M5 6c0-4 14-4 14 0v12c0 4-14 4-14 0V6Zm0 0c0 4 14 4 14 0',
 tube:'M4 7c0-6 16-6 16 0v10c0 6-16 6-16 0V7Zm0 0c0 6 16 6 16 0M9 7c0-2 6-2 6 0s-6 2-6 0',
 sketch:'m4 18 2-7 11-8 4 4-11 11-6 2ZM14 5l4 4M3 22h18',
 hole:'M4 5h16v14H4V5ZM8 12a4 4 0 1 0 8 0 4 4 0 0 0-8 0',
 union:'M3 5h11v6h7v10H10v-6H3V5Z',subtract:'M3 5h11v6H9v5H3V5ZM14 11h7v10H10v-5',
 intersect:'M3 5h11v11H3V5ZM10 11h11v10H10V11Z',
 undo:'M9 5 4 10l5 5M4 10h10a6 6 0 0 1 0 12',
 redo:'m15 5 5 5-5 5M20 10H10a6 6 0 0 0 0 12',
 save:'M4 3h14l3 3v15H3V3h1ZM7 3v6h10V3M7 21v-8h10v8',
 folder:'M3 6h7l2 3h9l-3 11H3V6Z',download:'M12 3v12m-5-5 5 5 5-5M4 16v5h16v-5',
 eye:'M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12ZM9 12a3 3 0 1 0 6 0 3 3 0 0 0-6 0',
 plus:'M12 5v14M5 12h14', grid:'M3 3h18v18H3V3ZM3 9h18M3 15h18M9 3v18M15 3v18',
 fit:'M8 3H3v5m13-5h5v5M3 16v5h5m13-5v5h-5',copy:'M8 8h13v13H8V8ZM16 8V3H3v13h5',
 trash:'M3 6h18M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7m4-7v7',
 chevron:'m9 5 7 7-7 7',check:'m5 12 4 4L19 6',help:'M9 8a3 3 0 1 1 5 2c-2 1-2 2-2 4M12 18h.01',
};
const icon=(name)=>'<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">'+('<path d="'+(icons[name]||icons.box)+'"/>')+'</svg>';
const tool=(action,label,i=action,extra='')=>'<button data-action="'+action+'" aria-label="'+label+'" '+extra+'>'+icon(i)+'<span>'+label+'</span></button>';
let project=demoProject(), selected=null, undo=[], redo=[], meshes=new Map(), wire=false;
let restored=false;
try { const raw=localStorage.getItem('solidcraft.project'); if(raw){project=validateProject(JSON.parse(raw));restored=true;} } catch {}
selected=project.bodies[0]?.id;
$('#app').innerHTML=`
<header class="topbar"><a class="brand" href="#" aria-label="SolidCraft"><span class="brand-mark">${icon('box')}</span>Solid<span>Craft</span><small>STUDIO</small></a><div class="project-title"><span class="title-dot"></span><button id="project-name" title="Renommer le projet"></button><span class="badge">LOCAL</span></div><div class="file-actions">${tool('open','Ouvrir','folder')}${tool('save','Enregistrer','save')}<button class="primary" data-action="export">${icon('download')} Exporter STL</button></div></header>
<nav class="ribbon"><div class="ribbon-title">CONCEPTION <span>01</span></div><div class="tool-group">${tool('sketch','Esquisse')}${tool('box','Bloc')}${tool('cylinder','Cylindre')}${tool('tube','Tube')}</div><div class="tool-group">${tool('hole','Perçage')}${tool('union','Union')}${tool('subtract','Soustraction')}${tool('intersect','Intersection')}</div><div class="tool-group history">${tool('undo','Annuler','undo','title="Annuler (Ctrl+Z)"')}${tool('redo','Rétablir','redo','title="Rétablir (Ctrl+Maj+Z)"')}</div><button class="help-button" data-action="help" title="Guide et raccourcis">${icon('help')}</button></nav>
<main><aside class="left-panel"><div class="panel-heading"><span>STRUCTURE DU PROJET</span><button data-action="new" title="Nouveau projet">${icon('plus')}</button></div><div class="tree-root">${icon('folder')}<strong id="tree-name"></strong><span>mm</span></div><div id="body-tree"></div><button class="add-body" data-action="box">${icon('plus')} Ajouter un solide</button><div class="left-bottom"><div class="small-label">VOTRE ATELIER, EN LOCAL</div><p>Des dimensions précises.<br>Des idées qui prennent forme.</p><button data-action="demo">Charger la pièce exemple <span>↗</span></button></div></aside>
<section class="viewport" aria-label="Vue 3D interactive"><div id="canvas-host"></div><div class="viewport-heading"><div class="eyebrow">ESPACE DE CONCEPTION</div><h1 id="viewport-name"></h1><div class="view-subtitle"><span></span> Perspective · Millimètres</div></div><div class="view-controls"><button data-action="view-top" title="Vue de dessus">DESSUS</button><button data-action="view-front" title="Vue de face">FACE</button><button class="cube" data-action="view-iso" title="Vue isométrique">${icon('box')}<span>ISO</span></button><button data-action="view-right" title="Vue de droite">DROITE</button></div><div class="viewport-tools">${tool('fit','Cadrer','fit','title="Cadrer les solides (F)"')}${tool('grid','Grille','grid','class="active"')}${tool('wire','Arêtes','box')}${tool('dimensions','Cotes','intersect')}</div><div id="dimension-overlay" hidden></div><div class="axis-legend"><span class="axis-z">Z</span><span class="axis-y">Y</span><span class="axis-x">X</span></div><div class="nav-hint"><span>Glisser</span> Orbite <i>·</i> <span>Clic droit</span> Déplacer <i>·</i> <span>Molette</span> Zoom</div><div id="empty-state" hidden><h2>Votre prochaine pièce commence ici.</h2><p>Ajoutez un volume ou dessinez une esquisse.</p><button class="primary" data-action="box">Créer mon premier solide</button></div></section>
<aside class="right-panel"><div class="panel-heading">PROPRIÉTÉS <span class="accent-dot"></span></div><div id="properties"></div><div class="print-card"><div>${icon('download')} DE LA CONCEPTION À L’OBJET</div><p>Exportez votre pièce en STL pour la préparer dans votre trancheur.</p><button data-action="export">Préparer l’export <span>↗</span></button></div></aside></main>
<footer><div><span class="status-dot"></span><span id="status">Prêt</span></div><div><span id="body-count"></span><i>•</i><span>Unités : mm</span><i>•</i><span>SolidCraft <b>0.1</b></span></div></footer>
<div id="toast" role="status"></div><dialog id="modal"><div id="modal-content"></div></dialog><input id="project-file" type="file" accept=".solidcraft,.json" hidden>
`;
const scene=new THREE.Scene(); scene.background=new THREE.Color('#171c24');
const camera=new THREE.PerspectiveCamera(36,1,0.1,50000); camera.up.set(0,0,1); camera.position.set(130,-180,150);
const renderer=new THREE.WebGLRenderer({antialias:true,alpha:false}); renderer.setPixelRatio(Math.min(devicePixelRatio,2)); renderer.outputColorSpace=THREE.SRGBColorSpace;
$('#canvas-host').appendChild(renderer.domElement);
const controls=new OrbitControls(camera,renderer.domElement); controls.enableDamping=true; controls.target.set(0,0,10); controls.minDistance=2; controls.maxDistance=20000;
scene.add(new THREE.HemisphereLight('#dce6ff','#56505c',1.7));
const sun=new THREE.DirectionalLight('#ffffff',2);sun.position.set(60,-100,160);scene.add(sun);
const rim=new THREE.DirectionalLight('#86aaff',1.8);rim.position.set(-80,70,70);scene.add(rim);
const grid=new THREE.GridHelper(1000,100,'#3d4655','#252d39');grid.rotation.x=Math.PI/2;grid.position.z=-0.08;grid.material.transparent=true;grid.material.opacity=0.5;scene.add(grid);
const axisMaterial=c=>new THREE.LineBasicMaterial({color:c,transparent:true,opacity:0.45});
const axes=new THREE.Group();
for(const [pts,color] of [[[-500,0,0,500,0,0],'#9d5965'],[[0,-500,0,0,500,0],'#4f8c79']]){
 const g=new THREE.BufferGeometry();g.setAttribute('position',new THREE.Float32BufferAttribute(pts,3));const l=new THREE.Line(g,axisMaterial(color));l.position.z=-0.04;axes.add(l);
}scene.add(axes);
const rootGroup=new THREE.Group();scene.add(rootGroup);
new ResizeObserver(()=>{const host=$('#canvas-host');camera.aspect=host.clientWidth/host.clientHeight;camera.updateProjectionMatrix();renderer.setSize(host.clientWidth,host.clientHeight);}).observe($('#canvas-host'));
renderer.setAnimationLoop(()=>{controls.update();renderer.render(scene,camera);});
function toast(text,error=false){const t=$('#toast');t.textContent=text;t.className='shown'+(error?' error':'');clearTimeout(toast.timer);toast.timer=setTimeout(()=>t.className='',4500);}
const current=()=>project.bodies.find(b=>b.id===selected);
function persist(){try{localStorage.setItem('solidcraft.project',JSON.stringify(project));$('#status').textContent='Sauvegardé sur cet appareil';}catch{$('#status').textContent='Sauvegarde locale indisponible — enregistrez le fichier';}}
function disposeGroup(){rootGroup.traverse(o=>{o.geometry?.dispose();if(o.material){(Array.isArray(o.material)?o.material:[o.material]).forEach(m=>m.dispose());}});rootGroup.clear();meshes.clear();}
function buildMeshes(candidate){
 const list=[];
 try{for(const b of candidate.bodies){const g=geometry(b);const mat=new THREE.MeshStandardMaterial({color:b.color,roughness:0.37,metalness:0.28,wireframe:wire});const mesh=new THREE.Mesh(g,mat);mesh.position.set(...b.position);mesh.visible=b.visible;mesh.userData.id=b.id;
 const edge=new THREE.LineSegments(new THREE.EdgesGeometry(g,25),new THREE.LineBasicMaterial({color:'#f4cd96',transparent:true,opacity:0.25}));edge.visible=b.holes.length===0&&b.type!=='boolean';mesh.add(edge);list.push(mesh);}}
 catch(e){list.forEach(m=>{m.geometry.dispose();m.material.dispose();m.children.forEach(c=>{c.geometry.dispose();c.material.dispose();});});throw e;}
 return list;
}
function setMeshes(list){disposeGroup();list.forEach(m=>{rootGroup.add(m);meshes.set(m.userData.id,m);});}
function commit(change){
 const before=clone(project), draft=clone(project);
 try{change(draft);validateProject(draft);const list=buildMeshes(draft);undo.push(before);if(undo.length>40)undo.shift();redo=[];project=draft;setMeshes(list);if(!current())selected=project.bodies[0]?.id;renderUI();persist();return true;}
 catch(e){toast(e.message||'Impossible de construire cette géométrie.',true);return false;}
}
function restoreHistory(from,to){if(!from.length)return;const next=from.at(-1);try{const list=buildMeshes(next);from.pop();to.push(clone(project));project=next;setMeshes(list);selected=project.bodies[0]?.id;renderUI();persist();}catch(e){toast(e.message,true);}}
function renderUI(){
 $('#project-name').textContent=project.name||'Sans titre';$('#tree-name').textContent=project.name||'Sans titre';$('#viewport-name').textContent=project.name||'Sans titre';
 $('#body-count').textContent=project.bodies.length+' solide'+(project.bodies.length>1?'s':'');
 $('#empty-state').hidden=project.bodies.length>0;
 $('[data-action="undo"]').disabled=!undo.length;$('[data-action="redo"]').disabled=!redo.length;
 $('#body-tree').innerHTML=project.bodies.map((b,i)=>`<div class="tree-item ${selected===b.id?'selected':''}"><button class="select-body" data-select="${b.id}">${icon(b.type)}<span>${esc(b.name)}</span><small>${String(i+1).padStart(2,'0')}</small></button><button class="visibility ${b.visible?'':'muted'}" data-visibility="${b.id}" title="${b.visible?'Masquer':'Afficher'} le solide" aria-label="${b.visible?'Masquer':'Afficher'} ${esc(b.name)}">${icon('eye')}</button></div>${selected===b.id?'<div class="feature-tree"><span>↳</span> '+esc(labels[b.type])+(b.holes.length?' <small>+ '+b.holes.length+' perçage(s)</small>':'')+'</div>':''}`).join('');
 for(const [id,m] of meshes){m.material.emissive.set(id===selected?'#352512':'#000000');m.children[0].material.opacity=id===selected?0.55:0.18;}
 renderProperties();updateDimensions();
 for(const a of ['hole','union','subtract','intersect'])$('[data-action="'+a+'"]').disabled=!current()||(['union','subtract','intersect'].includes(a)&&project.bodies.length<2);
}
const numberField=(label,key,value,min='0.1')=>`<label class="number-field"><span>${label}</span><div><input type="number" data-param="${key}" value="${value}" min="${min}" max="5000" step="any" required><span>mm</span></div></label>`;
function renderProperties(){
 const b=current();
 if(!b){$('#properties').innerHTML='<div class="property-empty">'+icon('box')+'<h3>Aucun solide sélectionné</h3><p>Créez un solide ou sélectionnez-le dans la vue 3D.</p></div>';return;}
 const p=b.params,mesh=meshes.get(b.id),size=new THREE.Vector3();mesh.geometry.boundingBox.getSize(size);
 const dim=b.type==='box'?numberField('Largeur · X','width',p.width)+numberField('Profondeur · Y','depth',p.depth)+numberField('Hauteur · Z','height',p.height):b.type==='cylinder'||b.type==='tube'?numberField('Rayon extérieur','radius',p.radius)+(b.type==='tube'?numberField('Rayon intérieur','innerRadius',p.innerRadius):'')+numberField('Hauteur · Z','height',p.height):b.type==='sketch'?numberField('Hauteur d’extrusion','height',p.height)+'<button class="text-button" data-action="edit-sketch">Modifier le contour ↗</button>':'<p class="property-note">Résultat d’une opération entre deux solides. Annulez pour retrouver les sources.</p>';
 $('#properties').innerHTML=`<div class="selected-heading"><div class="solid-icon">${icon(b.type)}</div><div><span>SOLIDE SÉLECTIONNÉ</span><input id="body-name" value="${esc(b.name)}" maxlength="100" aria-label="Nom du solide"></div><input id="body-color" type="color" value="${b.color}" title="Couleur du solide" aria-label="Couleur du solide"></div><section class="property-section"><h3>Dimensions <span>mm</span></h3>${dim}</section><section class="property-section"><h3>Position</h3><div class="position-fields">${['X','Y','Z'].map((v,i)=>`<label><span class="axis-${v.toLowerCase()}">${v}</span><input type="number" data-position="${i}" value="${b.position[i]}" step="any" min="-5000" max="5000" aria-label="Position ${v}"></label>`).join('')}</div></section><section class="property-section"><h3>Perçages traversants <span>${b.holes.length}</span></h3>${b.holes.length?b.holes.map((h,i)=>`<div class="hole-row"><button data-edit-hole="${i}">${icon('hole')}<span>Ø ${h.diameter} <small>X ${h.x} · Y ${h.y}</small></span></button><button data-delete-hole="${i}" title="Supprimer ce perçage">${icon('trash')}</button></div>`).join(''):'<p class="property-note">Ajoutez un trou suivant l’axe Z.</p>'}<button class="dashed-button" data-action="hole">${icon('plus')} Ajouter un perçage</button></section><section class="property-section"><h3>Informations</h3><dl><dt>Encombrement</dt><dd>${size.toArray().map(n=>n.toFixed(1)).join(' × ')}</dd><dt>Volume</dt><dd>${(volume(mesh.geometry)/1000).toLocaleString('fr-FR',{maximumFractionDigits:2})} cm³</dd></dl></section><div class="body-actions">${tool('duplicate','Dupliquer','copy')}${tool('delete','Supprimer','trash')}</div>`;
}
function fit(){const box=new THREE.Box3();for(const m of meshes.values())if(m.visible)box.expandByObject(m);if(box.isEmpty()){controls.target.set(0,0,0);camera.position.set(130,-180,150);return;}const size=box.getSize(new THREE.Vector3()),center=box.getCenter(new THREE.Vector3());const direction=camera.position.clone().sub(controls.target).normalize();const dist=Math.max(size.length(),10)/(2*Math.tan(THREE.MathUtils.degToRad(camera.fov/2)))*1.25/Math.min(camera.aspect,1);controls.target.copy(center);camera.position.copy(center).addScaledVector(direction,dist);controls.update();}
function updateDimensions(){const b=current(),el=$('#dimension-overlay');if(!b){el.textContent='Sélectionnez un solide pour afficher ses cotes.';return;}const s=meshes.get(b.id).geometry.boundingBox.getSize(new THREE.Vector3());el.textContent='Encombrement  X '+s.x.toFixed(2)+' × Y '+s.y.toFixed(2)+' × Z '+s.z.toFixed(2)+' mm';}
function modal(title,subtitle,content,onSubmit,submit='Appliquer'){
 const dlg=$('#modal');if(dlg.open)dlg.close();
 $('#modal-content').innerHTML=`<form id="dialog-form"><div class="modal-header"><div><span class="eyebrow">SOLIDCRAFT / CONCEPTION</span><h2>${title}</h2></div><button type="button" class="close-modal" aria-label="Fermer">×</button></div><p class="modal-subtitle">${subtitle}</p>${content}<div id="dialog-error" role="alert"></div><div class="modal-footer"><button type="button" class="close-modal">Annuler</button><button class="primary" type="submit">${submit}</button></div></form>`;
 dlg.showModal();$('#dialog-form').onsubmit=e=>{e.preventDefault();try{if(onSubmit(new FormData(e.currentTarget))!==false)dlg.close();}catch(err){$('#dialog-error').textContent=err.message;}};
 dlg.querySelectorAll('.close-modal').forEach(b=>b.onclick=()=>dlg.close());
}
const input=(name,label,value,min=-5000)=>`<label class="dialog-field">${label}<input name="${name}" type="number" value="${value}" min="${min}" max="5000" step="any" required></label>`;
function holeDialog(index=null){const b=current();if(!b)return;const h=index===null?{diameter:6,x:0,y:0}:b.holes[index];
 modal(index===null?'Nouveau perçage':'Modifier le perçage','Trou traversant suivant Z. Les coordonnées X et Y sont relatives au solide.',`<div class="dialog-grid">${input('diameter','Diamètre (mm)',h.diameter,0.1)}${input('x','Centre X (mm)',h.x)}${input('y','Centre Y (mm)',h.y)}</div>`,data=>commit(p=>{const target=p.bodies.find(x=>x.id===b.id),hole=Object.fromEntries(['diameter','x','y'].map(k=>[k,Number(data.get(k))]));if(index===null)target.holes.push(hole);else target.holes[index]=hole;}));
}
function booleanDialog(operation){const b=current();if(!b)return;const names={union:'Union de solides',subtract:'Soustraction de solides',intersect:'Intersection de solides'};
 modal(names[operation],operation==='subtract'?'Le second solide est retiré du solide sélectionné. Les deux sources seront remplacées par le résultat.':'Les deux solides sources seront remplacés par le résultat. Ils doivent se recouper pour une intersection.',`<div class="boolean-source">Solide de départ <strong>${esc(b.name)}</strong></div><label class="dialog-field">Second solide<select name="target">${project.bodies.filter(x=>x.id!==b.id).map(x=>'<option value="'+x.id+'">'+esc(x.name)+'</option>').join('')}</select></label>`,data=>{const id=uid();const ok=commit(p=>{const a=p.bodies.find(x=>x.id===b.id),other=p.bodies.find(x=>x.id===data.get('target'));p.bodies=p.bodies.filter(x=>x.id!==a.id&&x.id!==other.id);p.bodies.push({id,name:names[operation],type:'boolean',params:{},sources:[a,other],operation,position:[0,0,0],holes:[],visible:true,color:a.color});});if(ok){selected=id;renderUI();}return ok;},'Construire le solide');
}
function sketchDialog(edit=false){
 const b=edit?current():null;let points=b?clone(b.params.points):[];const scale=4,w=560,h=320;
 modal(edit?'Modifier l’esquisse':'Dessiner une esquisse','Cliquez pour placer les sommets sur le plan XY (grille de 5 mm). Le contour se ferme automatiquement. Évitez les croisements.',`<canvas id="sketch-canvas" width="560" height="320"></canvas><div class="sketch-actions"><button type="button" id="sketch-back">Retirer un point</button><button type="button" id="sketch-clear">Effacer</button><span id="point-count"></span></div>${input('height','Hauteur d’extrusion (mm)',b?.params.height||12,0.1)}`,data=>{
 if(!validPolygon(points))throw new Error('Placez au moins 3 points formant un contour sans croisement.');
 if(edit)return commit(p=>{const item=p.bodies.find(x=>x.id===b.id);item.params.points=points;item.params.height=Number(data.get('height'));});
 const item=body('sketch',{points,height:Number(data.get('height'))});const ok=commit(p=>p.bodies.push(item));if(ok){selected=item.id;renderUI();fit();}return ok;},'Extruder le contour');
 const canvas=$('#sketch-canvas'),ctx=canvas.getContext('2d');
 const draw=()=>{ctx.clearRect(0,0,w,h);ctx.fillStyle='#141a23';ctx.fillRect(0,0,w,h);ctx.strokeStyle='#29313e';ctx.lineWidth=1;
 for(let x=0;x<=w;x+=20){ctx.beginPath();ctx.moveTo(x,0);ctx.lineTo(x,h);ctx.stroke();}for(let y=0;y<=h;y+=20){ctx.beginPath();ctx.moveTo(0,y);ctx.lineTo(w,y);ctx.stroke();}
 ctx.strokeStyle='#646c7d';ctx.beginPath();ctx.moveTo(w/2,0);ctx.lineTo(w/2,h);ctx.moveTo(0,h/2);ctx.lineTo(w,h/2);ctx.stroke();ctx.fillStyle='#8f9aaf';ctx.font='11px sans-serif';ctx.fillText('X →',w-35,h/2-8);ctx.fillText('Y ↑',w/2+8,16);
 if(points.length){ctx.beginPath();points.forEach(([x,y],i)=>i?ctx.lineTo(w/2+x*scale,h/2-y*scale):ctx.moveTo(w/2+x*scale,h/2-y*scale));if(points.length>2)ctx.closePath();ctx.fillStyle='#edaf6830';ctx.fill();ctx.strokeStyle='#edaf68';ctx.lineWidth=2;ctx.stroke();points.forEach(([x,y],i)=>{ctx.beginPath();ctx.arc(w/2+x*scale,h/2-y*scale,4,0,Math.PI*2);ctx.fillStyle='#ffc384';ctx.fill();ctx.fillText(String(i+1),w/2+x*scale+7,h/2-y*scale-7);});}
 $('#point-count').textContent=points.length+' points · pas 5 mm';};
 canvas.onclick=e=>{const r=canvas.getBoundingClientRect();const x=Math.round(((e.clientX-r.left)*w/r.width-w/2)/scale/5)*5,y=Math.round((h/2-(e.clientY-r.top)*h/r.height)/scale/5)*5;if(points.length<100)points.push([x,y]);draw();};$('#sketch-back').onclick=()=>{points.pop();draw();};$('#sketch-clear').onclick=()=>{points=[];draw();};draw();
}
function download(blob,name){const url=URL.createObjectURL(blob),a=document.createElement('a');a.href=url;a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}
const filename=()=> (project.name||'piece').replace(/[^a-zA-Z0-9À-ÿ_-]+/g,'-');
function exportDialog(){
 if(!project.bodies.length){toast('Créez un solide avant de l’exporter.',true);return;}
 modal('Exporter pour l’impression 3D','Le STL contient un maillage, sans les paramètres de conception. Importez-le en millimètres dans votre trancheur.',`<label class="dialog-field">Solides à exporter<select name="scope">${current()?'<option value="selected">Solide sélectionné — '+esc(current().name)+'</option>':''}<option value="visible">Tous les solides visibles</option></select></label><div class="export-note">STL binaire · résolution circulaire fine<br>Pour obtenir une seule pièce à partir de volumes qui se chevauchent, utilisez Union avant l’export.</div>`,data=>{
 const list=data.get('scope')==='selected'?[current()]:project.bodies.filter(b=>b.visible);if(!list.length)throw new Error('Aucun solide visible à exporter.');
 const group=new THREE.Group();list.forEach(b=>{const m=meshes.get(b.id);const copy=new THREE.Mesh(m.geometry);copy.position.copy(m.position);group.add(copy);});group.updateMatrixWorld(true);
 const stl=new STLExporter().parse(group,{binary:true});download(new Blob([stl],{type:'application/octet-stream'}),filename()+'.stl');toast('STL exporté — importez-le en millimètres.');},'Télécharger le STL');
}
const actions={
 box:()=>add('box'),cylinder:()=>add('cylinder'),tube:()=>add('tube'),sketch:()=>sketchDialog(), 'edit-sketch':()=>sketchDialog(true),
 hole:()=>holeDialog(),union:()=>booleanDialog('union'),subtract:()=>booleanDialog('subtract'),intersect:()=>booleanDialog('intersect'),
 undo:()=>restoreHistory(undo,redo),redo:()=>restoreHistory(redo,undo),fit,
 grid:()=>{grid.visible=!grid.visible;axes.visible=grid.visible;$('[data-action="grid"]').classList.toggle('active',grid.visible);},
 wire:()=>{wire=!wire;meshes.forEach(m=>m.material.wireframe=wire);$('[data-action="wire"]').classList.toggle('active',wire);},
 dimensions:()=>{$('#dimension-overlay').hidden=!$('#dimension-overlay').hidden;$('[data-action="dimensions"]').classList.toggle('active',!$('#dimension-overlay').hidden);},
 'view-iso':()=>setView([1,-1.4,1]),'view-top':()=>setView([0,-0.0001,1]),'view-front':()=>setView([0,-1,0]),'view-right':()=>setView([1,0,0]),
 duplicate:()=>{if(!current())return;const b=clone(current());b.id=uid();b.name+=' — copie';b.position[0]+=15;b.position[1]+=15;if(commit(p=>p.bodies.push(b))){selected=b.id;renderUI();}},
 delete:()=>{if(current())commit(p=>p.bodies=p.bodies.filter(b=>b.id!==selected));},
 save:()=>{download(new Blob([JSON.stringify(project,null,2)],{type:'application/json'}),filename()+'.solidcraft');toast('Projet enregistré avec ses paramètres.');},
 open:()=>$('#project-file').click(),export:exportDialog,
 new:()=>modal('Nouveau projet','Le projet actuel reste accessible avec Annuler. Enregistrez un fichier pour le conserver durablement.','<label class="dialog-field">Nom du projet<input name="name" value="Nouvelle pièce" maxlength="100" required></label>',data=>commit(p=>{p.name=data.get('name');p.bodies=[];}),'Créer le projet'),
 demo:()=>modal('Charger la pièce exemple','La platine de fixation remplace le projet courant. Vous pouvez revenir en arrière avec Annuler.','',()=>{const ok=commit(p=>Object.assign(p,demoProject()));if(ok)fit();return ok;},'Charger la platine'),
 help:()=>modal('Bienvenue dans votre atelier','Le parcours de conception, en quelques gestes.',`<ol class="help-list"><li><strong>Créez un volume.</strong> Choisissez Bloc, Cylindre ou Tube, ou tracez un contour avec Esquisse.</li><li><strong>Ajustez la pièce.</strong> Modifiez ses dimensions et sa position dans le panneau de droite.</li><li><strong>Usinez les détails.</strong> Ajoutez des perçages Z ou soustrayez un solide d’un autre.</li><li><strong>Conservez et imprimez.</strong> Enregistrez le projet paramétrique, puis exportez un STL pour le trancheur.</li></ol><div class="export-note">F : cadrer · Suppr : supprimer · Ctrl+Z : annuler<br>Ctrl+Maj+Z : rétablir · Ctrl+S : enregistrer<br><br>Version 0.1 : géométrie par maillages. Pas encore de congés, de contraintes d’esquisse, de STEP ni d’assemblages mécaniques.</div>`,()=>true,'Commencer'),
};
function add(type){const b=body(type);b.name=labels[type]+' '+(project.bodies.length+1);if(commit(p=>p.bodies.push(b))){selected=b.id;renderUI();fit();toast(b.name+' créé — ajustez ses dimensions à droite.');}}
function setView(direction){const dist=camera.position.distanceTo(controls.target);camera.position.copy(controls.target).addScaledVector(new THREE.Vector3(...direction).normalize(),dist);controls.update();}
document.addEventListener('click',e=>{
 const action=e.target.closest('[data-action]');if(action&&!action.disabled)actions[action.dataset.action]?.();
 const select=e.target.closest('[data-select]');if(select){selected=select.dataset.select;renderUI();}
 const vis=e.target.closest('[data-visibility]');if(vis)commit(p=>{const b=p.bodies.find(x=>x.id===vis.dataset.visibility);b.visible=!b.visible;});
 const edit=e.target.closest('[data-edit-hole]');if(edit)holeDialog(Number(edit.dataset.editHole));
 const del=e.target.closest('[data-delete-hole]');if(del)commit(p=>p.bodies.find(x=>x.id===selected).holes.splice(Number(del.dataset.deleteHole),1));
});
$('#properties').addEventListener('change',e=>{
 const target=e.target,b=current();if(!b)return;
 if(target.matches('[data-param],[data-position]')&&(!target.validity.valid||!Number.isFinite(Number(target.value))||target.value==='')){toast('Saisissez une dimension valide.',true);renderProperties();return;}
 const ok=commit(p=>{const item=p.bodies.find(x=>x.id===b.id);if(target.dataset.param)item.params[target.dataset.param]=Number(target.value);else if(target.dataset.position!==undefined)item.position[Number(target.dataset.position)]=Number(target.value);else if(target.id==='body-name')item.name=target.value.trim()||labels[item.type];else if(target.id==='body-color')item.color=target.value;});
 if(!ok)renderProperties();
});
$('#project-name').onclick=()=>modal('Renommer le projet','Donnez un nom à votre pièce.',`<label class="dialog-field">Nom<input name="name" maxlength="100" value="${esc(project.name)}" required></label>`,data=>commit(p=>p.name=data.get('name').trim()||'Sans titre'));
$('#project-file').onchange=async e=>{
 const file=e.target.files[0];if(!file)return;
 try{if(file.size>2e6)throw new Error('Le fichier dépasse la limite de 2 Mo.');const imported=validateProject(JSON.parse(await file.text()));if(typeof imported.name!=='string'||imported.name.length>100)throw new Error('Nom du projet invalide.');if(commit(p=>Object.assign(p,imported))){fit();toast('Projet chargé.');}}catch(err){toast(err.message,true);}finally{e.target.value='';}
};
let down=null;renderer.domElement.addEventListener('pointerdown',e=>down=[e.clientX,e.clientY,e.button]);
renderer.domElement.addEventListener('pointerup',e=>{if(!down||down[2]!==0||Math.hypot(e.clientX-down[0],e.clientY-down[1])>4)return;const rect=renderer.domElement.getBoundingClientRect();const pointer=new THREE.Vector2((e.clientX-rect.left)/rect.width*2-1,-(e.clientY-rect.top)/rect.height*2+1);const ray=new THREE.Raycaster();ray.setFromCamera(pointer,camera);const hit=ray.intersectObjects([...meshes.values()].filter(m=>m.visible),false)[0];selected=hit?.object.userData.id;renderUI();});
document.addEventListener('keydown',e=>{if(e.target.matches('input,textarea,select')||$('#modal').open)return;const mod=e.ctrlKey||e.metaKey;if(mod&&e.key.toLowerCase()==='z'){e.preventDefault();actions[e.shiftKey?'redo':'undo']();}else if(mod&&e.key.toLowerCase()==='s'){e.preventDefault();actions.save();}else if(e.key==='Delete')actions.delete();else if(e.key.toLowerCase()==='f')fit();});
try{setMeshes(buildMeshes(project));}catch{project=demoProject();selected=project.bodies[0].id;setMeshes(buildMeshes(project));toast('La sauvegarde locale était illisible. Pièce exemple chargée.',true);}
renderUI();requestAnimationFrame(fit);persist();
if(restored)toast('Votre dernier projet a été restauré.');
