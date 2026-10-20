import test from 'node:test';
import assert from 'node:assert/strict';
import * as THREE from 'three';
import { STLExporter } from 'three/addons/exporters/STLExporter.js';
import { STLLoader } from 'three/addons/loaders/STLLoader.js';
import { body, geometry, volume, validateProject, validPolygon, demoProject } from '../src/model.js';

const close=(actual,expected,tolerance=0.005)=>assert.ok(Math.abs(actual-expected)/expected<tolerance,`${actual} ≈ ${expected}`);
test('bloc : dimensions en millimètres et volume',()=>{
 const g=geometry(body('box',{width:80,depth:50,height:8}));
 assert.deepEqual(g.boundingBox.min.toArray(),[-40,-25,0]);
 assert.deepEqual(g.boundingBox.max.toArray(),[40,25,8]);close(volume(g),32000);g.dispose();
});
test('cylindre et tube : axe Z et volume',()=>{
 for(const type of ['cylinder','tube']){
 const b=body(type,{radius:20,innerRadius:15,height:30}),g=geometry(b);
 close(volume(g),Math.PI*(400-(type==='tube'?225:0))*30);
 assert.ok(Math.abs(g.boundingBox.min.z)<1e-5);assert.equal(g.boundingBox.max.z,30);g.dispose();
 }
});
test('quatre perçages : retrait de matière et export STL relisible',()=>{
 const p=demoProject(),g=geometry(p.bodies[0]);close(volume(g),32000-4*Math.PI*9*8);
 const mesh=new THREE.Mesh(g);mesh.updateMatrixWorld(true);
 const data=new STLExporter().parse(mesh,{binary:true});
 assert.equal(data.byteLength,84+data.getUint32(80,true)*50);
 const loaded=new STLLoader().parse(data.buffer);loaded.computeBoundingBox();
 loaded.boundingBox.min.toArray().forEach((v,i)=>assert.ok(Math.abs(v-[-40,-25,0][i])<1e-6));
 assert.deepEqual(loaded.boundingBox.max.toArray(),[40,25,8]);
 close(volume(loaded),volume(g),0.00001);
 for(const value of loaded.attributes.position.array)assert.ok(Number.isFinite(value));
 g.dispose();loaded.dispose();
});
test('opérations booléennes respectent les positions des sources',()=>{
 for(const [operation,expected] of [['union',1500],['subtract',500],['intersect',500]]){
 const a=body('box',{width:10,depth:10,height:10}),b=body('box',{width:10,depth:10,height:10});b.position=[5,0,0];
 const combined={...body('box'),type:'boolean',params:{},sources:[a,b],operation};
 const g=geometry(combined);close(volume(g),expected);g.dispose();
 }
});
test('intersection vide refusée',()=>{
 const a=body('box'),b=body('box');b.position=[1000,0,0];
 assert.throws(()=>geometry({...a,type:'boolean',sources:[a,b],operation:'intersect'}),/vide/);
});
test('esquisse concave extrudée et contours invalides',()=>{
 const points=[[-25,-15],[25,-15],[25,5],[5,5],[5,25],[-25,25]];
 assert.ok(validPolygon(points));
 const g=geometry(body('sketch',{points,height:12}));close(volume(g),19200);g.dispose();
 assert.equal(validPolygon([[0,0],[10,10],[0,10],[10,0]]),false);
 assert.equal(validPolygon([[0,0],[10,0],[20,0]]),false);
 assert.equal(validPolygon([[0,0],[0,0],[10,10]]),false);
});
test('import paramétrique : aller-retour, limites et valeurs non finies',()=>{
 const project=demoProject();assert.deepEqual(validateProject(JSON.parse(JSON.stringify(project))),project);
 const tube=body('tube',{innerRadius:21,radius:20});assert.throws(()=>validateProject({...project,bodies:[tube]}),/rayon/);
 const invalid=structuredClone(project);invalid.bodies[0].position[0]=Infinity;assert.throws(()=>validateProject(invalid));
 assert.throws(()=>validateProject({...project,version:2}));
 assert.throws(()=>validateProject({...project,bodies:[project.bodies[0],project.bodies[0]]}),/dupliqués/);
});
