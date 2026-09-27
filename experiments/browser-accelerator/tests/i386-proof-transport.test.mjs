import assert from "node:assert/strict";
import {
  collectDualProofs,
  deriveRecursiveSource,
  projectV3ProofToCompact,
  validateCompactAgainstV3,
} from "../web/i386-proof-transport.mjs";

function fixture() {
  const memory="A-memory#test";
  const visual=[
    {key:`${memory}:L1`,startKey:`${memory}:L1`,endKey:`${memory}:L1`,localHandle:1,label:"root",tags:["root"]},
    {key:`${memory}:L2`,startKey:`${memory}:L2`,endKey:`${memory}:L1`,localHandle:2,label:"start",tags:["start"]},
    {key:`${memory}:L3`,startKey:`${memory}:L1`,endKey:`${memory}:L2`,localHandle:3,label:null,tags:[]},
  ];
  return {
    schemaVersion:3,
    block:"fixture",
    prepare:{
      compilerLabel:"fixture compiler",
      runtimeMemoryExists:false,
      compiledLinks:2,
      carrierDuplets:[{start:1,end:1},{start:2,end:1}],
      semanticRoots:[
        {role:"root",carrierRef:1,source:"8"},
        {role:"start",carrierRef:2,source:"98"},
      ],
      theoryAdmissions:["L2"],
    },
    load:{
      memoryInstanceId:memory,linksBeforeLoad:1,linksAfterLoad:2,importedDuplets:2,carrierRoundTrip:true,
      semanticRoots:[
        {role:"root",carrierRef:1,source:"8",localHandle:1},
        {role:"start",carrierRef:2,source:"98",localHandle:2},
      ],
    },
    execute:{
      memoryInstanceId:memory,
      reactions:[{
        memoryInstanceId:memory,step:0,scopeBefore:["L2"],rawRuleMatches:1,
        transitionedMembers:1,handoffCount:0,scopeAfter:["L3"],linksAfter:3,quiescent:true,
      }],
      activeReactionCount:0,finalQuiescent:true,
    },
    result:{
      memoryInstanceId:memory,resultAnum:"16898",resultSequenceAnum:"16898",
      decodedValue:1,oracleValue:1,oracleMatches:true,linksFinal:3,
      identicalRerunLinkDelta:0,visualLinks:visual,
    },
  };
}
function clone(value) { return JSON.parse(JSON.stringify(value)); }
function wasmFor(proof,compact) {
  const encoder=new TextEncoder();
  const p=encoder.encode(JSON.stringify(proof));
  const c=encoder.encode(JSON.stringify(compact));
  const memory={buffer:new ArrayBuffer(p.length+c.length+32)};
  new Uint8Array(memory.buffer,8,p.length).set(p);
  new Uint8Array(memory.buffer,8+p.length,c.length).set(c);
  return {
    memory,
    amemory_i386_lab_proof_available:()=>1,
    amemory_i386_lab_proof_json_len:()=>p.length,
    amemory_i386_lab_proof_json_ptr:()=>8,
    amemory_i386_lab_compact_proof_available:()=>1,
    amemory_i386_lab_compact_proof_json_len:()=>c.length,
    amemory_i386_lab_compact_proof_json_ptr:()=>8+p.length,
  };
}

{
  const proof=fixture();
  assert.equal(deriveRecursiveSource(proof.prepare.carrierDuplets,1),"8");
  assert.equal(deriveRecursiveSource(proof.prepare.carrierDuplets,2),"98");
  const compact=projectV3ProofToCompact(proof);
  assert.equal(compact.representationId,"amemory-proof-compact-json");
  assert.deepEqual(compact.topology.base,{starts:[1,2],ends:[1,1]});
  assert.deepEqual(compact.topology.append,{starts:[1],ends:[2]});
  assert.deepEqual(compact.roots,[{role:"root",carrierRef:1},{role:"start",carrierRef:2}]);
  assert.equal(validateCompactAgainstV3(proof,compact),compact);
  const pair=collectDualProofs(wasmFor(proof,compact));
  assert.deepEqual(pair.proof,proof);
  assert.deepEqual(pair.compactProof,compact);
}
{
  const proof=fixture();
  const compact=projectV3ProofToCompact(proof);
  compact.topology.append.ends[0]=1;
  assert.throws(()=>validateCompactAgainstV3(proof,compact),/differs from independent browser projection/);
}
{
  const proof=fixture();
  const compact=projectV3ProofToCompact(proof);
  compact.representationVersion="9.9.9";
  assert.throws(()=>validateCompactAgainstV3(proof,compact),/unsupported compact version/);
}
{
  const proof=fixture();
  proof.prepare.semanticRoots[1].source="8";
  assert.throws(()=>projectV3ProofToCompact(proof),/root directory mismatch|derived root witness mismatch/);
}
{
  const proof=fixture();
  const compact=projectV3ProofToCompact(proof);
  const wasm=wasmFor(proof,compact);
  wasm.amemory_i386_lab_compact_proof_available=()=>0;
  assert.throws(()=>collectDualProofs(wasm),/availability differs/);
}
{
  const proof=fixture();
  proof.execute.reactions[0].memoryInstanceId="A-memory#other";
  assert.throws(()=>projectV3ProofToCompact(proof),/changed A-memory identity/);
}
{
  const proof=fixture();
  const compact=clone(projectV3ProofToCompact(proof));
  compact.roots.push({role:"extra",carrierRef:1});
  assert.throws(()=>validateCompactAgainstV3(proof,compact),/differs from independent browser projection/);
}
console.log("i386 browser dual-proof transport witnesses: PASS");
