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
    const linksBefore=previousLinks;
    const linksAfter=uint(step.linksAfter,`reaction[${index}].linksAfter`,linksBefore,finalLinks);
    previousLinks=linksAfter;
    return {
      step:index,
      scopeBefore:array(step.scopeBefore,`reaction[${index}].scopeBefore`)
        .map((value,i)=>localRef(value,linksBefore,`reaction[${index}].scopeBefore[${i}]`)),
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


function bool(value,label) {
  if (typeof value !== "boolean") fail(label + " must be boolean");
  return value;
}

function decodeCompactProof(compact) {
  object(compact,"compact proof");
  if (compact.schemaVersion!==COMPACT_SCHEMA) fail("unsupported compact schema " + compact.schemaVersion);
  if (compact.representationId!==COMPACT_ID) fail("unexpected compact representation " + compact.representationId);
  if (compact.representationVersion!==COMPACT_VERSION) fail("unsupported compact version " + compact.representationVersion);
  if (compact.sourceProofSchemaVersion!==V3_SCHEMA) fail("unexpected compact source schema " + compact.sourceProofSchemaVersion);

  const block=text(compact.block,"compact.block");
  const memoryId=text(compact.memoryInstanceId,"compact.memoryInstanceId");
  const prepare=object(compact.prepare,"compact.prepare");
  const compiledLinks=uint(prepare.compiledLinks,"compact.prepare.compiledLinks",1);
  bool(prepare.runtimeMemoryExists,"compact.prepare.runtimeMemoryExists");

  const topology=object(compact.topology,"compact.topology");
  const base=object(topology.base,"compact.topology.base");
  const baseStarts=array(base.starts,"compact.topology.base.starts");
  const baseEnds=array(base.ends,"compact.topology.base.ends");
  if (baseStarts.length!==baseEnds.length) fail("compact base topology columns differ in length");
  if (baseStarts.length!==compiledLinks) fail("compact base topology/compiledLinks mismatch");
  const baseLen=baseStarts.length;

  const append=object(topology.append,"compact.topology.append");
  const appendStarts=array(append.starts,"compact.topology.append.starts");
  const appendEnds=array(append.ends,"compact.topology.append.ends");
  if (appendStarts.length!==appendEnds.length) fail("compact append topology columns differ in length");
  const finalLinks=baseLen+appendStarts.length;

  const basePairs=baseStarts.map((start,index)=>({
    start:uint(start,"compact base L"+(index+1)+".start",1,baseLen),
    end:uint(baseEnds[index],"compact base L"+(index+1)+".end",1,baseLen),
  }));
  const appendPairs=appendStarts.map((start,index)=>({
    start:uint(start,"compact append L"+(baseLen+index+1)+".start",1,finalLinks),
    end:uint(appendEnds[index],"compact append L"+(baseLen+index+1)+".end",1,finalLinks),
  }));
  const allPairs=basePairs.concat(appendPairs);

  const overlayByHandle=new Map();
  array(topology.overlays,"compact.topology.overlays").forEach((overlay,index)=>{
    object(overlay,"compact overlay["+index+"]");
    const handle=uint(overlay.localHandle,"compact overlay["+index+"].localHandle",1,finalLinks);
    if (overlayByHandle.has(handle)) fail("duplicate compact overlay for L"+handle);
    if (overlay.label!=null && typeof overlay.label!=="string") fail("compact overlay L"+handle+" label must be string/null");
    const tags=array(overlay.tags,"compact overlay L"+handle+" tags");
    if (!tags.every((tag)=>typeof tag==="string")) fail("compact overlay L"+handle+" tags must be strings");
    overlayByHandle.set(handle,{localHandle:handle,label:overlay.label??null,tags:[...tags]});
  });

  const seenRoles=new Set();
  const roots=array(compact.roots,"compact.roots").map((root,index)=>{
    object(root,"compact root["+index+"]");
    const role=text(root.role,"compact root["+index+"].role");
    if (seenRoles.has(role)) fail("duplicate compact root role "+role);
    seenRoles.add(role);
    const carrierRef=uint(root.carrierRef,role+".carrierRef",1,baseLen);
    deriveRecursiveSource(basePairs,carrierRef);
    return {role,carrierRef};
  });

  const theoryAdmissions=array(compact.theoryAdmissions,"compact.theoryAdmissions")
    .map((handle,index)=>uint(handle,"compact Theory["+index+"]",1,baseLen));

  const load=object(compact.load,"compact.load");
  const linksBeforeLoad=uint(load.linksBeforeLoad,"compact.load.linksBeforeLoad");
  const linksAfterLoad=uint(load.linksAfterLoad,"compact.load.linksAfterLoad");
  const importedDuplets=uint(load.importedDuplets,"compact.load.importedDuplets");
  if (linksAfterLoad!==baseLen || importedDuplets!==baseLen || load.carrierRoundTrip!==true) {
    fail("compact load/carrier summary mismatch");
  }

  const execute=object(compact.execute,"compact.execute");
  const sourceReactions=array(execute.reactions,"compact.execute.reactions");
  let previousLinks=linksAfterLoad;
  const reactions=sourceReactions.map((step,index)=>{
    object(step,"compact reaction["+index+"]");
    if (step.step!==index) fail("compact reaction["+index+"] step index mismatch");
    const linksBefore=previousLinks;
    const linksAfter=uint(step.linksAfter,"compact reaction["+index+"].linksAfter",linksBefore,finalLinks);
    const scopeBefore=array(step.scopeBefore,"compact reaction["+index+"].scopeBefore")
      .map((handle,i)=>uint(handle,"compact reaction["+index+"].scopeBefore["+i+"]",1,linksBefore));
    const scopeAfter=array(step.scopeAfter,"compact reaction["+index+"].scopeAfter")
      .map((handle,i)=>uint(handle,"compact reaction["+index+"].scopeAfter["+i+"]",1,linksAfter));
    const reaction={
      step:index,
      scopeBefore,
      rawRuleMatches:uint(step.rawRuleMatches,"compact reaction["+index+"].rawRuleMatches"),
      transitionedMembers:uint(step.transitionedMembers,"compact reaction["+index+"].transitionedMembers"),
      handoffCount:uint(step.handoffCount,"compact reaction["+index+"].handoffCount"),
      scopeAfter,
      linksAfter,
      quiescent:bool(step.quiescent,"compact reaction["+index+"].quiescent"),
    };
    previousLinks=linksAfter;
    return reaction;
  });
  const activeReactionCount=uint(execute.activeReactionCount,"compact.execute.activeReactionCount");
  const active=reactions.filter((step)=>!step.quiescent).length;
  if (activeReactionCount!==active) fail("compact activeReactionCount/trace mismatch");
  const finalQuiescent=bool(execute.finalQuiescent,"compact.execute.finalQuiescent");
  const traceQuiescent=reactions.length>0 && reactions.at(-1).quiescent===true;
  if (finalQuiescent!==traceQuiescent) fail("compact finalQuiescent/trace mismatch");

  const result=object(compact.result,"compact.result");
  if (Object.prototype.hasOwnProperty.call(result,"memoryInstanceId") ||
      Object.prototype.hasOwnProperty.call(result,"visualLinks")) {
    fail("compact result must not duplicate runtime identity or visual topology");
  }
  const resultLinksFinal=uint(result.linksFinal,"compact.result.linksFinal",baseLen);
  if (resultLinksFinal!==finalLinks) fail("compact topology/result linksFinal mismatch");
  const rerunDelta=uint(result.identicalRerunLinkDelta,"compact.result.identicalRerunLinkDelta");
  if (previousLinks+rerunDelta!==finalLinks) fail("compact reaction/rerun counts do not reach linksFinal");

  return {
    compact,
    block,
    memoryId,
    prepare,
    baseLen,
    finalLinks,
    basePairs,
    appendPairs,
    allPairs,
    overlayByHandle,
    roots,
    theoryAdmissions,
    load:{linksBeforeLoad,linksAfterLoad,importedDuplets,carrierRoundTrip:true},
    execute:{activeReactionCount,finalQuiescent,reactions},
    result,
  };
}

export function validateCompactProof(compact) {
  decodeCompactProof(compact);
  return compact;
}

export function inflateCompactProof(compact) {
  const decoded=decodeCompactProof(compact);
  const preparedRoots=decoded.roots.map((root)=>({
    role:root.role,
    carrierRef:root.carrierRef,
    source:deriveRecursiveSource(decoded.basePairs,root.carrierRef),
  }));
  const loadedRoots=preparedRoots.map((root)=>({
    ...root,
    localHandle:root.carrierRef,
  }));
  const visualLinks=decoded.allPairs.map((pair,index)=>{
    const handle=index+1;
    const overlay=decoded.overlayByHandle.get(handle);
    return {
      key:decoded.memoryId+":L"+handle,
      startKey:decoded.memoryId+":L"+pair.start,
      endKey:decoded.memoryId+":L"+pair.end,
      localHandle:handle,
      label:overlay?.label??null,
      tags:overlay?.tags??[],
    };
  });
  return {
    schemaVersion:V3_SCHEMA,
    block:decoded.block,
    prepare:{
      runtimeMemoryExists:decoded.prepare.runtimeMemoryExists,
      compiledLinks:decoded.prepare.compiledLinks,
      carrierDuplets:decoded.basePairs.map((pair)=>({...pair})),
      semanticRoots:preparedRoots,
      theoryAdmissions:decoded.theoryAdmissions.map((handle)=>"L"+handle),
    },
    load:{
      memoryInstanceId:decoded.memoryId,
      ...decoded.load,
      semanticRoots:loadedRoots,
    },
    execute:{
      memoryInstanceId:decoded.memoryId,
      activeReactionCount:decoded.execute.activeReactionCount,
      finalQuiescent:decoded.execute.finalQuiescent,
      reactions:decoded.execute.reactions.map((step)=>({
        memoryInstanceId:decoded.memoryId,
        step:step.step,
        scopeBefore:step.scopeBefore.map((handle)=>"L"+handle),
        rawRuleMatches:step.rawRuleMatches,
        transitionedMembers:step.transitionedMembers,
        handoffCount:step.handoffCount,
        scopeAfter:step.scopeAfter.map((handle)=>"L"+handle),
        linksAfter:step.linksAfter,
        quiescent:step.quiescent,
      })),
    },
    result:{
      ...decoded.result,
      memoryInstanceId:decoded.memoryId,
      visualLinks,
    },
  };
}

function normalizeRendererProof(proof) {
  const normalized=JSON.parse(JSON.stringify(proof));
  if (normalized?.prepare) delete normalized.prepare.compilerLabel;
  return normalized;
}

export function validateInflatedCompactAgainstV3(proof,compact) {
  validateCompactAgainstV3(proof,compact);
  const inflated=inflateCompactProof(compact);
  if (stable(normalizeRendererProof(inflated))!==stable(normalizeRendererProof(proof))) {
    fail("compact renderer projection differs from schema-v3 after presentation normalization");
  }
  return inflated;
}

export function selectBrowserProof({proof=null,compactProof=null}={}) {
  if (compactProof!==null) {
    const rendererProof=proof===null
      ? inflateCompactProof(compactProof)
      : validateInflatedCompactAgainstV3(proof,compactProof);
    return {
      proof:rendererProof,
      compactProof,
      v3Proof:proof,
      transport:"compact",
    };
  }
  if (proof!==null) {
    object(proof,"schema-v3 fallback proof");
    if (proof.schemaVersion!==V3_SCHEMA) fail("unsupported fallback schema "+proof.schemaVersion);
    return {
      proof,
      compactProof:null,
      v3Proof:proof,
      transport:"v3-fallback",
    };
  }
  return {
    proof:null,
    compactProof:null,
    v3Proof:null,
    transport:"none",
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

export function collectBrowserProof(wasm) {
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
  return selectBrowserProof({proof,compactProof});
}

export function collectDualProofs(wasm) {
  const selected=collectBrowserProof(wasm);
  return {proof:selected.v3Proof,compactProof:selected.compactProof};
}
