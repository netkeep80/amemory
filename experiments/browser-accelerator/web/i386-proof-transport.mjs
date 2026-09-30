import { readJsonAbi } from "./i386-wasm-json.mjs";

const LEGACY_SOURCE_SCHEMA = 3;
const SOURCE_SCHEMA = 4;
const LEGACY_COMPACT_SCHEMA = 1;
const COMPACT_SCHEMA = 2;
const COMPACT_ID = "amemory-proof-compact-json";
const LEGACY_COMPACT_VERSION = "0.1.0";
const COMPACT_VERSION = "0.2.0";

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

function bool(value,label) {
  if (typeof value !== "boolean") fail(label + " must be boolean");
  return value;
}

function decodeCompactProof(compact) {
  object(compact,"compact proof");
  const compactSchema=compact.schemaVersion;
  if (compactSchema!==LEGACY_COMPACT_SCHEMA && compactSchema!==COMPACT_SCHEMA) {
    fail("unsupported compact schema " + compactSchema);
  }
  if (compact.representationId!==COMPACT_ID) fail("unexpected compact representation " + compact.representationId);

  const legacy=compactSchema===LEGACY_COMPACT_SCHEMA;
  const expectedVersion=legacy ? LEGACY_COMPACT_VERSION : COMPACT_VERSION;
  const expectedSourceSchema=legacy ? LEGACY_SOURCE_SCHEMA : SOURCE_SCHEMA;
  if (compact.representationVersion!==expectedVersion) {
    fail("unsupported compact version " + compact.representationVersion + " for schema " + compactSchema);
  }
  if (compact.sourceProofSchemaVersion!==expectedSourceSchema) {
    fail("unexpected compact source schema " + compact.sourceProofSchemaVersion + " for compact schema " + compactSchema);
  }

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
  const linksBeforeExecute=uint(
    execute.linksBeforeExecute ?? linksAfterLoad,
    "compact.execute.linksBeforeExecute",
    linksAfterLoad,
    finalLinks,
  );
  const sourceReactions=array(execute.reactions,"compact.execute.reactions");
  let previousLinks=linksBeforeExecute;
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
  const hasLegacyResult=Object.prototype.hasOwnProperty.call(result,"resultAnum");
  const hasRecursiveWire=Object.prototype.hasOwnProperty.call(result,"resultRecursiveWire");
  let resultRecursiveWire;
  if (legacy) {
    if (!hasLegacyResult || hasRecursiveWire) {
      fail("legacy compact v1 result must contain resultAnum only");
    }
    resultRecursiveWire=text(result.resultAnum,"compact.result.resultAnum");
  } else {
    if (!hasRecursiveWire || hasLegacyResult) {
      fail("compact v2 result must contain resultRecursiveWire only");
    }
    resultRecursiveWire=text(result.resultRecursiveWire,"compact.result.resultRecursiveWire");
  }
  text(result.resultSequenceAnum,"compact.result.resultSequenceAnum");
  const resultLinksFinal=uint(result.linksFinal,"compact.result.linksFinal",baseLen);
  if (resultLinksFinal!==finalLinks) fail("compact topology/result linksFinal mismatch");
  const rerunDelta=uint(result.identicalRerunLinkDelta,"compact.result.identicalRerunLinkDelta");
  if (previousLinks+rerunDelta!==finalLinks) fail("compact reaction/rerun counts do not reach linksFinal");

  return {
    compact,
    compactSchema,
    sourceProofSchemaVersion:expectedSourceSchema,
    resultRecursiveWire,
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
    execute:{linksBeforeExecute,activeReactionCount,finalQuiescent,reactions},
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
    schemaVersion:decoded.sourceProofSchemaVersion,
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
      linksBeforeExecute:decoded.execute.linksBeforeExecute,
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

export function collectBrowserProof(wasm) {
  const compactProof=readJsonAbi(wasm,{
    available:"amemory_i386_lab_compact_proof_available",
    length:"amemory_i386_lab_compact_proof_json_len",
    pointer:"amemory_i386_lab_compact_proof_json_ptr",
    byte:"amemory_i386_lab_compact_proof_json_byte",
  },"compact proof");
  if (compactProof===null) {
    return {proof:null,compactProof:null,transport:"none"};
  }
  return {
    proof:inflateCompactProof(compactProof),
    compactProof,
    transport:"compact",
  };
}
