const V3_SCHEMA = 3;
const COMPACT_SCHEMA = 1;
const COMPACT_ID = "amemory-proof-compact-json";
const COMPACT_VERSION = "0.1.0";

function fail(message) { throw new Error(`proof transport: ${message}`); }
function object(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) fail(`${label} must be an object`);
  return value;
}
function array(value, label) {
  if (!Array.isArray(value)) fail(`${label} must be an array`);
  return value;
}
function uint(value, label, min=0, max=0xffff_ffff) {
  if (!Number.isInteger(value) || value < min || value > max) fail(`${label} must be an integer in ${min}..${max}`);
  return value;
}
function text(value, label) {
  if (typeof value !== "string" || value.length === 0) fail(`${label} must be a non-empty string`);
  return value;
}
function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(Object.keys(value).sort().map((key)=>[key,canonical(value[key])]));
}
function stable(value) { return JSON.stringify(canonical(value)); }
function localRef(value,max,label) {
  if (typeof value !== "string" || !/^L\d+$/.test(value)) fail(`${label} must be L<n>`);
  return uint(Number(value.slice(1)),label,1,max);
}
function visualRef(value,memoryId,max,label) {
  const prefix=`${memoryId}:L`;
  if (typeof value !== "string" || !value.startsWith(prefix) || !/^\d+$/.test(value.slice(prefix.length))) {
    fail(`${label} is not a Link reference in ${memoryId}`);
  }
  return uint(Number(value.slice(prefix.length)),label,1,max);
}

export function deriveRecursiveSource(basePairs,rootHandle) {
  array(basePairs,"base topology");
  uint(rootHandle,"root handle",1,basePairs.length);
  const output=[];
  const active=new Set();
  const stack=[{kind:"visit",handle:rootHandle}];
  while (stack.length) {
    const frame=stack.pop();
    if (frame.kind==="exit") { active.delete(frame.handle); continue; }
    const handle=uint(frame.handle,"recursive handle",1,basePairs.length);
    const pair=object(basePairs[handle-1],`base L${handle}`);
    const start=uint(pair.start,`L${handle}.start`,1,basePairs.length);
    const end=uint(pair.end,`L${handle}.end`,1,basePairs.length);
    if (start===handle && end===handle) { output.push("8"); continue; }
    if (active.has(handle)) fail(`recursive witness L${handle} is non-well-founded`);
    active.add(handle);
    stack.push({kind:"exit",handle});
    if (start===handle) {
      output.push("9"); stack.push({kind:"visit",handle:end});
    } else if (end===handle) {
      output.push("6"); stack.push({kind:"visit",handle:start});
    } else {
      output.push("1");
      stack.push({kind:"visit",handle:end});
      stack.push({kind:"visit",handle:start});
    }
  }
  return output.join("");
}

export function projectV3ProofToCompact(proof) {
  object(proof,"schema-v3 proof");
  if (proof.schemaVersion!==V3_SCHEMA) fail(`unsupported source schema ${proof.schemaVersion}`);
  const prepare=object(proof.prepare,"prepare");
  const load=object(proof.load,"load");
  const execute=object(proof.execute,"execute");
  const result=object(proof.result,"result");
  const memoryId=text(result.memoryInstanceId,"result.memoryInstanceId");
  if (load.memoryInstanceId!==memoryId || execute.memoryInstanceId!==memoryId) fail("v3 stages changed runtime A-memory identity");

  const carrier=array(prepare.carrierDuplets,"prepare.carrierDuplets");
  const baseLen=carrier.length;
  if (uint(prepare.compiledLinks,"prepare.compiledLinks")!==baseLen) fail("compiledLinks/carrier length mismatch");
  if (load.linksAfterLoad!==baseLen || load.importedDuplets!==baseLen || load.carrierRoundTrip!==true) fail("load/carrier summary mismatch");

  const basePairs=carrier.map((duplet,index)=>{
    object(duplet,`carrier[${index}]`);
    return {
      start:uint(duplet.start,`carrier[${index}].start`,1,baseLen),
      end:uint(duplet.end,`carrier[${index}].end`,1,baseLen),
    };
  });
  const base={starts:basePairs.map((p)=>p.start),ends:basePairs.map((p)=>p.end)};

  const visual=array(result.visualLinks,"result.visualLinks");
  const finalLinks=uint(result.linksFinal,"result.linksFinal",baseLen);
  if (visual.length!==finalLinks) fail("visualLinks/linksFinal mismatch");
  const append={starts:[],ends:[]};
  const overlays=[];
  visual.forEach((link,index)=>{
    object(link,`visual[${index}]`);
    const handle=index+1;
    if (link.localHandle!==handle) fail(`visual topology is not dense at L${handle}`);
    if (visualRef(link.key,memoryId,finalLinks,`L${handle}.key`)!==handle) fail(`visual key mismatch at L${handle}`);
    const start=visualRef(link.startKey,memoryId,finalLinks,`L${handle}.startKey`);
    const end=visualRef(link.endKey,memoryId,finalLinks,`L${handle}.endKey`);
    if (handle<=baseLen) {
      if (basePairs[index].start!==start || basePairs[index].end!==end) fail(`prepared prefix mismatch at L${handle}`);
    } else {
      append.starts.push(start); append.ends.push(end);
    }
    if (link.label!=null && typeof link.label!=="string") fail(`L${handle}.label must be string/null`);
    const tags=array(link.tags,`L${handle}.tags`);
    if (!tags.every((tag)=>typeof tag==="string")) fail(`L${handle}.tags must be strings`);
    if (link.label!=null || tags.length) overlays.push({localHandle:handle,label:link.label??null,tags:[...tags]});
  });

  const preparedRoots=array(prepare.semanticRoots,"prepare.semanticRoots");
  const loadedRoots=array(load.semanticRoots,"load.semanticRoots");
  if (preparedRoots.length!==loadedRoots.length) fail("prepared/loaded root count mismatch");
  const loadedByRole=new Map();
  loadedRoots.forEach((root)=>{
    object(root,"loaded root");
    const role=text(root.role,"loaded root role");
    if (loadedByRole.has(role)) fail(`duplicate loaded root role ${role}`);
    loadedByRole.set(role,root);
  });
  const seenRoles=new Set();
  const roots=preparedRoots.map((root)=>{
    object(root,"prepared root");
    const role=text(root.role,"prepared root role");
    if (seenRoles.has(role)) fail(`duplicate prepared root role ${role}`);
    seenRoles.add(role);
    const ref=uint(root.carrierRef,`${role}.carrierRef`,1,baseLen);
    const loaded=loadedByRole.get(role);
    if (!loaded || loaded.carrierRef!==ref || loaded.localHandle!==ref || loaded.source!==root.source) fail(`root directory mismatch for ${role}`);
    if (deriveRecursiveSource(basePairs,ref)!==root.source) fail(`derived root witness mismatch for ${role}`);
    return {role,carrierRef:ref};
  });

  const theoryAdmissions=array(prepare.theoryAdmissions,"prepare.theoryAdmissions")
    .map((value,index)=>localRef(value,baseLen,`Theory[${index}]`));
  const sourceReactions=array(execute.reactions,"execute.reactions");
  const active=sourceReactions.filter((step)=>!step.quiescent).length;
  if (execute.activeReactionCount!==active) fail("activeReactionCount/trace mismatch");
  const lastQuiescent=sourceReactions.length>0 && sourceReactions.at(-1).quiescent===true;
  if (execute.finalQuiescent!==lastQuiescent) fail("finalQuiescent/trace mismatch");

  let previousLinks=load.linksAfterLoad;
  const reactions=sourceReactions.map((step,index)=>{
    object(step,`reaction[${index}]`);
    if (step.memoryInstanceId!==memoryId) fail(`reaction[${index}] changed A-memory identity`);
    if (step.step!==index) fail(`reaction[${index}] step index mismatch`);
    const linksAfter=uint(step.linksAfter,`reaction[${index}].linksAfter`,previousLinks,finalLinks);
    previousLinks=linksAfter;
    return {
      step:index,
      scopeBefore:array(step.scopeBefore,`reaction[${index}].scopeBefore`)
        .map((value,i)=>localRef(value,linksAfter,`reaction[${index}].scopeBefore[${i}]`)),
      rawRuleMatches:uint(step.rawRuleMatches,`reaction[${index}].rawRuleMatches`),
      transitionedMembers:uint(step.transitionedMembers,`reaction[${index}].transitionedMembers`),
      handoffCount:uint(step.handoffCount,`reaction[${index}].handoffCount`),
      scopeAfter:array(step.scopeAfter,`reaction[${index}].scopeAfter`)
        .map((value,i)=>localRef(value,linksAfter,`reaction[${index}].scopeAfter[${i}]`)),
      linksAfter,
      quiescent:step.quiescent===true,
    };
  });
  const rerunDelta=uint(result.identicalRerunLinkDelta,"result.identicalRerunLinkDelta");
  if (previousLinks+rerunDelta!==finalLinks) fail("reaction/rerun Link counts do not reach linksFinal");

  const {memoryInstanceId,visualLinks,...compactResult}=result;
  void visualLinks;
  if (memoryInstanceId!==memoryId) fail("result memory identity mismatch");
  return {
    schemaVersion:COMPACT_SCHEMA,
    representationId:COMPACT_ID,
    representationVersion:COMPACT_VERSION,
    sourceProofSchemaVersion:V3_SCHEMA,
    block:proof.block,
    memoryInstanceId:memoryId,
    prepare:{runtimeMemoryExists:prepare.runtimeMemoryExists,compiledLinks:prepare.compiledLinks},
    topology:{base,append,overlays},
    roots,
    theoryAdmissions,
    load:{
      linksBeforeLoad:load.linksBeforeLoad,
      linksAfterLoad:load.linksAfterLoad,
      importedDuplets:load.importedDuplets,
      carrierRoundTrip:load.carrierRoundTrip,
    },
    execute:{activeReactionCount:execute.activeReactionCount,finalQuiescent:execute.finalQuiescent,reactions},
    result:compactResult,
  };
}

export function validateCompactAgainstV3(proof,compact) {
  object(compact,"compact proof");
  if (compact.schemaVersion!==COMPACT_SCHEMA) fail(`unsupported compact schema ${compact.schemaVersion}`);
  if (compact.representationId!==COMPACT_ID) fail(`unexpected compact representation ${compact.representationId}`);
  if (compact.representationVersion!==COMPACT_VERSION) fail(`unsupported compact version ${compact.representationVersion}`);
  const expected=projectV3ProofToCompact(proof);
  if (stable(compact)!==stable(expected)) fail("compact proof differs from independent browser projection of schema-v3");
  return compact;
}

function readJsonAbi(wasm,names,label) {
  const available=wasm[names.available];
  if (typeof available!=="function" || available()!==1) return null;
  const length=wasm[names.length];
  if (typeof length!=="function") fail(`${label} length ABI missing`);
  const len=length()>>>0;
  if (!len) fail(`${label} ABI reported empty JSON`);
  let bytes;
  const pointer=wasm[names.pointer];
  if (wasm.memory && typeof pointer==="function") {
    const ptr=pointer()>>>0;
    if (ptr>wasm.memory.buffer.byteLength || len>wasm.memory.buffer.byteLength-ptr) fail(`${label} pointer outside WASM memory`);
    bytes=new Uint8Array(wasm.memory.buffer,ptr,len);
  } else {
    const byte=wasm[names.byte];
    if (typeof byte!=="function") fail(`${label} byte ABI missing`);
    bytes=new Uint8Array(len);
    for (let i=0;i<len;i+=1) {
      const value=byte(i)>>>0;
      if (value>255) fail(`${label} invalid byte at ${i}`);
      bytes[i]=value;
    }
  }
  try { return JSON.parse(new TextDecoder().decode(bytes)); }
  catch (error) { fail(`${label} JSON parse failed: ${error.message}`); }
}

export function collectDualProofs(wasm) {
  const proof=readJsonAbi(wasm,{
    available:"amemory_i386_lab_proof_available",
    length:"amemory_i386_lab_proof_json_len",
    pointer:"amemory_i386_lab_proof_json_ptr",
    byte:"amemory_i386_lab_proof_json_byte",
  },"schema-v3 proof");
  const compactProof=readJsonAbi(wasm,{
    available:"amemory_i386_lab_compact_proof_available",
    length:"amemory_i386_lab_compact_proof_json_len",
    pointer:"amemory_i386_lab_compact_proof_json_ptr",
    byte:"amemory_i386_lab_compact_proof_json_byte",
  },"compact proof");
  if ((proof===null)!==(compactProof===null)) fail("v3/compact ABI availability differs");
  if (proof!==null) validateCompactAgainstV3(proof,compactProof);
  return {proof,compactProof};
}
