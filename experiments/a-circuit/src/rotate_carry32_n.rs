use super::{
    flag_patch::{
        install_alu_effect_result_tag, set_flag_action,
        undefined_flag_action, FlagPatchSchema,
    },
    full_adder::{
        call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
    },
    logic_n::{GateSet, LogicProgram},
    proof_n::{
        execute_session_to_quiescence, execute_to_quiescence,
        identical_rerun, load_runtime, load_runtime_session, loaded_handle,
        prepare_stage, semantic_source, theory_admissions, visual_snapshot,
        ProofRuntimeMemory, ProofRuntimeSession, WebProofLoadStage,
        WebProofPrepareStage, WebProofResultStage, WebStructuralProof,
    },
};
use amemory_optimized_cpu_probe::{
    structural::{materialize_exact_sequence, read_exact_sequence},
    Handle, OptimizedLinkStore, ROOT_HANDLE,
};

const WIDTH: usize = 32;
const EXT_WIDTH: usize = 33;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind { Rcl, Rcr }

struct AnchorGen {
    current: Handle,
    flip: bool,
    o: Handle,
    c: Handle,
}
impl AnchorGen {
    fn new(store:&mut OptimizedLinkStore, mut seed:Handle, o:Handle, c:Handle)->Self{
        for pole in [c,c,o,c,o,o,c,c,o,o,c,o,c,c,o] {
            seed=store.ensure_pair(seed,pole).unwrap();
        }
        Self{current:seed,flip:false,o,c}
    }
    fn next(&mut self,store:&mut OptimizedLinkStore)->Handle{
        let pole=if self.flip {self.c}else{self.o};
        self.flip=!self.flip;
        self.current=store.ensure_pair(self.current,pole).unwrap();
        self.current
    }
    fn roles(&mut self,store:&mut OptimizedLinkStore,n:usize)->Vec<Handle>{
        (0..n).map(|_|self.next(store)).collect()
    }
}
fn stage_frame(store:&mut OptimizedLinkStore,tag:Handle,values:&[Handle])->Handle{
    let payload=materialize_exact_sequence(store,values).unwrap();
    let d=store.ensure_pair(tag,payload).unwrap();
    store.ensure_start_self_closed(d).unwrap()
}
fn binary_call(f:&mut FullFixture,function:Handle,a:Handle,b:Handle)->Handle{
    let args=materialize_exact_sequence(&mut f.store,&[a,b]).unwrap();
    call(&mut f.store,f.apply,function,args)
}

#[derive(Clone,Debug)]
struct Program{
    rcl:Handle,
    rcr:Handle,
    result_tag:Handle,
    schema:FlagPatchSchema,
    gates:GateSet,
    links_after_build:usize,
}

fn rotated(input:&[Handle],cf_in:Handle,kind:Kind,count:usize)->(Vec<Handle>,Handle){
    assert_eq!(input.len(),WIDTH);
    assert!((1..32).contains(&count));
    let mut ext=Vec::with_capacity(EXT_WIDTH);
    ext.extend_from_slice(input);
    ext.push(cf_in);
    let mut out=Vec::with_capacity(EXT_WIDTH);
    match kind {
        Kind::Rcl=>{
            out.extend_from_slice(&ext[EXT_WIDTH-count..]);
            out.extend_from_slice(&ext[..EXT_WIDTH-count]);
        }
        Kind::Rcr=>{
            out.extend_from_slice(&ext[count..]);
            out.extend_from_slice(&ext[..count]);
        }
    }
    let cf_out=out[WIDTH];
    (out[..WIDTH].to_vec(),cf_out)
}

fn install_rule(
    f:&mut FullFixture,
    a:&mut AnchorGen,
    function:Handle,
    kind:Kind,
    masked:usize,
    of_tag:Handle,
    result_tag:Handle,
    schema:FlagPatchSchema,
    gates:GateSet,
){
    let k=a.next(&mut f.store);
    let bits=a.roles(&mut f.store,WIDTH);
    let cf_in=a.next(&mut f.store);
    let hi=a.roles(&mut f.store,3);

    let mut cb=Vec::with_capacity(8);
    for bit in 0..5 {
        cb.push(if (masked>>bit)&1==1 {f.one}else{f.zero});
    }
    cb.extend_from_slice(&hi);

    let word=materialize_exact_sequence(&mut f.store,&bits).unwrap();
    let count=materialize_exact_sequence(&mut f.store,&cb).unwrap();
    let args=materialize_exact_sequence(&mut f.store,&[word,count,cf_in]).unwrap();
    let invocation=call(&mut f.store,f.apply,function,args);
    let before=f.store.ensure_pair(k,invocation).unwrap();

    let after=if masked==0 {
        let payload=materialize_exact_sequence(&mut f.store,&[f.one,word,ROOT_HANDLE]).unwrap();
        let envelope=f.store.ensure_pair(result_tag,payload).unwrap();
        f.store.ensure_pair(k,envelope).unwrap()
    } else {
        let (out,cf_out)=rotated(&bits,cf_in,kind,masked);
        let out_word=materialize_exact_sequence(&mut f.store,&out).unwrap();

        if masked==1 {
            let mut state=Vec::with_capacity(WIDTH+2);
            state.push(k); state.push(cf_out); state.extend_from_slice(&out);
            let caller=stage_frame(&mut f.store,of_tag,&state);
            let (x,y)=match kind {
                Kind::Rcl=>(out[WIDTH-1],cf_out),
                Kind::Rcr=>(out[WIDTH-1],out[WIDTH-2]),
            };
            let of_call=binary_call(f,gates.xor2,x,y);
            f.store.ensure_pair(caller,of_call).unwrap()
        } else {
            let set_cf=set_flag_action(&mut f.store,schema,schema.cf,cf_out);
            let undef_of=undefined_flag_action(&mut f.store,schema,schema.of);
            let patch=materialize_exact_sequence(&mut f.store,&[set_cf,undef_of]).unwrap();
            let payload=materialize_exact_sequence(&mut f.store,&[f.one,out_word,patch]).unwrap();
            let envelope=f.store.ensure_pair(result_tag,payload).unwrap();
            f.store.ensure_pair(k,envelope).unwrap()
        }
    };

    let mut roles=Vec::with_capacity(1+WIDTH+1+3);
    roles.push(k); roles.extend_from_slice(&bits); roles.push(cf_in); roles.extend_from_slice(&hi);
    let (_,admission)=define_bundle_rule(&mut f.store,f.theory,&roles,before,&[after]);
    index_rule_for(&mut f.store,&[f.o],admission);
}

impl Program{
    fn install(f:&mut FullFixture)->Self{
        let logic=LogicProgram::install(f,WIDTH);
        let gates=logic.gates;
        let seed=f.store.ensure_pair(logic.result_tag,logic.word_binary).unwrap();
        let mut a=AnchorGen::new(&mut f.store,seed,f.o,f.c);

        let l=a.next(&mut f.store); let r=a.next(&mut f.store);
        let rcl=f.store.ensure_pair(l,r).unwrap();
        let l=a.next(&mut f.store); let r=a.next(&mut f.store);
        let rcr=f.store.ensure_pair(l,r).unwrap();
        let l=a.next(&mut f.store); let r=a.next(&mut f.store);
        let of_tag=f.store.ensure_pair(l,r).unwrap();

        let shared=f.store.ensure_pair(f.k,f.full).unwrap();
        let schema=FlagPatchSchema::install(&mut f.store,shared,f.o,f.c);
        let result_tag=install_alu_effect_result_tag(&mut f.store,shared,f.o,f.c);

        for (kind,function) in [(Kind::Rcl,rcl),(Kind::Rcr,rcr)] {
            for masked in 0..32 {
                install_rule(f,&mut a,function,kind,masked,of_tag,result_tag,schema,gates);
            }
        }

        {
            let k=a.next(&mut f.store);
            let cf=a.next(&mut f.store);
            let bits=a.roles(&mut f.store,WIDTH);
            let of=a.next(&mut f.store);
            let mut state=Vec::with_capacity(WIDTH+2);
            state.push(k); state.push(cf); state.extend_from_slice(&bits);
            let caller=stage_frame(&mut f.store,of_tag,&state);
            let of_result=materialize_exact_sequence(&mut f.store,&[of]).unwrap();
            let before=f.store.ensure_pair(caller,of_result).unwrap();

            let word=materialize_exact_sequence(&mut f.store,&bits).unwrap();
            let set_cf=set_flag_action(&mut f.store,schema,schema.cf,cf);
            let set_of=set_flag_action(&mut f.store,schema,schema.of,of);
            let patch=materialize_exact_sequence(&mut f.store,&[set_cf,set_of]).unwrap();
            let payload=materialize_exact_sequence(&mut f.store,&[f.one,word,patch]).unwrap();
            let envelope=f.store.ensure_pair(result_tag,payload).unwrap();
            let after=f.store.ensure_pair(k,envelope).unwrap();

            let mut roles=state; roles.push(of);
            let (_,admission)=define_bundle_rule(&mut f.store,f.theory,&roles,before,&[after]);
            index_rule_for(&mut f.store,&gates.bit_outputs,admission);
        }

        Self{rcl,rcr,result_tag,schema,gates,links_after_build:f.store.link_count()}
    }
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum FlagState{Preserve,Set(u8),Undefined}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
struct Outcome{
    writeback:u8,
    value:u32,
    cf:FlagState,
    of:FlagState,
    patch_len:usize,
    reactions:usize,
}

fn bit_handles(f:&FullFixture,width:usize,value:u32)->Vec<Handle>{
    (0..width).map(|i|if (value>>i)&1==1{f.one}else{f.zero}).collect()
}
fn decode_bit(f:&FullFixture,h:Handle)->u8{
    if h==f.one {1}else{assert_eq!(h,f.zero);0}
}
fn decode_word(f:&FullFixture,h:Handle)->u32{
    let bits=read_exact_sequence(&f.store,h).unwrap(); assert_eq!(bits.len(),WIDTH);
    let mut v=0u32;
    for (i,b) in bits.into_iter().enumerate(){v|=u32::from(decode_bit(f,b))<<i;}
    v
}
fn decode_action(f:&FullFixture,s:FlagPatchSchema,a:Handle,flag:Handle)->FlagState{
    let (tag,payload)=f.store.poles(a).unwrap();
    let vals=read_exact_sequence(&f.store,payload).unwrap();
    if tag==s.set_tag {
        assert_eq!(vals.len(),2); assert_eq!(vals[0],flag);
        FlagState::Set(decode_bit(f,vals[1]))
    } else {
        assert_eq!(tag,s.undefined_tag); assert_eq!(vals,vec![flag]);
        FlagState::Undefined
    }
}
fn decode(f:&FullFixture,p:&Program,reactions:usize)->Outcome{
    assert_eq!(f.engine.current().len(),1);
    let (caller,endpoint)=f.store.poles(f.engine.current()[0]).unwrap();
    assert_eq!(caller,f.k);
    let (tag,payload)=f.store.poles(endpoint).unwrap(); assert_eq!(tag,p.result_tag);
    let vals=read_exact_sequence(&f.store,payload).unwrap(); assert_eq!(vals.len(),3);
    let patch=read_exact_sequence(&f.store,vals[2]).unwrap();
    if patch.is_empty(){
        Outcome{writeback:decode_bit(f,vals[0]),value:decode_word(f,vals[1]),
            cf:FlagState::Preserve,of:FlagState::Preserve,patch_len:0,reactions}
    } else {
        assert_eq!(patch.len(),2);
        Outcome{writeback:decode_bit(f,vals[0]),value:decode_word(f,vals[1]),
            cf:decode_action(f,p.schema,patch[0],p.schema.cf),
            of:decode_action(f,p.schema,patch[1],p.schema.of),patch_len:2,reactions}
    }
}
fn run(f:&mut FullFixture,p:&Program,function:Handle,value:u32,count:u8,cf_in:u8)->Outcome{
    let wb=bit_handles(f,WIDTH,value);
    let cb=bit_handles(f,8,u32::from(count));
    let word=materialize_exact_sequence(&mut f.store,&wb).unwrap();
    let cw=materialize_exact_sequence(&mut f.store,&cb).unwrap();
    let cfh=if cf_in==0{f.zero}else{f.one};
    let args=materialize_exact_sequence(&mut f.store,&[word,cw,cfh]).unwrap();
    let inv=call(&mut f.store,f.apply,function,args);
    let initial=f.store.ensure_pair(f.k,inv).unwrap();
    f.engine.set_current(&f.store,&[initial]).unwrap();

    let mut reactions=0;
    loop {
        let x=f.engine.run(&mut f.store).unwrap();
        if x.quiescent {assert_eq!(x.raw_rule_matches,0);break;}
        reactions+=1;
        assert_eq!(x.raw_rule_matches,1); assert_eq!(x.transitioned_members,1);
        assert_eq!(x.handoff_count,1); assert_eq!(x.next_members.len(),1);
        assert!(reactions<=4);
    }
    decode(f,p,reactions)
}
fn expected(p:&Program,function:Handle,value:u32,count:u8,cf_in:u8)->Outcome{
    let k=u32::from(count&31);
    if k==0 {
        return Outcome{writeback:1,value,cf:FlagState::Preserve,of:FlagState::Preserve,
            patch_len:0,reactions:1};
    }
    let mask=(1u64<<33)-1;
    let ext=(u64::from(cf_in)<<32)|u64::from(value);
    let rotated=if function==p.rcl {
        ((ext<<k)|(ext>>(33-k)))&mask
    } else {
        assert_eq!(function,p.rcr);
        ((ext>>k)|(ext<<(33-k)))&mask
    };
    let result=rotated as u32;
    let cf=((rotated>>32)&1) as u8;
    let of=if k==1 {
        if function==p.rcl {
            FlagState::Set((((result>>31)&1) as u8)^cf)
        } else {
            FlagState::Set((((result>>31)&1) as u8)^(((result>>30)&1) as u8))
        }
    } else {FlagState::Undefined};
    Outcome{writeback:1,value:result,cf:FlagState::Set(cf),of,patch_len:2,
        reactions:if k==1{3}else{1}}
}
fn counts()->[u8;16]{[0,1,2,7,8,15,16,30,31,32,33,63,64,65,95,255]}
fn values()->Vec<u32>{
    let mut v=vec![0,1,0x8000_0000,u32::MAX,0xaaaa_aaaa,0x5555_5555,0x8000_0001,0xa5a5_5a5a];
    let mut z=0x85eb_ca6bu32;
    for _ in 0..5 {z=z.wrapping_mul(1664525).wrapping_add(1013904223);v.push(z);}
    v.sort_unstable();v.dedup();v
}


#[derive(Clone,Debug)]
pub(crate) struct WebRotateCarryProofExecution{
    pub(crate) outcome:WebRotateCarryOutcome,
    pub(crate) proof:WebStructuralProof,
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) struct WebRotateCarryOutcome{
    pub(crate) value:u32,
    pub(crate) writeback:u8,
    pub(crate) defined_mask:u32,
    pub(crate) value_mask:u32,
    pub(crate) undefined_mask:u32,
    pub(crate) preserve_mask:u32,
    pub(crate) reactions:u32,
    pub(crate) links_after_build:u32,
    pub(crate) links_after_first:u32,
    pub(crate) steady_link_delta:u32,
    pub(crate) quiescent:u8,
}

const WEB_CF:u32=1<<0;
const WEB_PF:u32=1<<2;
const WEB_AF:u32=1<<4;
const WEB_ZF:u32=1<<6;
const WEB_SF:u32=1<<7;
const WEB_OF:u32=1<<11;
const WEB_OTHER_STATUS:u32=WEB_PF|WEB_AF|WEB_ZF|WEB_SF;

fn web_rotate_carry_state(mask:u32,state:FlagState,defined:&mut u32,values:&mut u32,undefined:&mut u32,preserve:&mut u32){
    match state{
        FlagState::Set(bit)=>{*defined|=mask;if bit!=0{*values|=mask;}}
        FlagState::Undefined=>*undefined|=mask,
        FlagState::Preserve=>*preserve|=mask,
    }
}

fn runtime_rotate_carry_bit(
    memory:&ProofRuntimeMemory,
    value:Handle,
    zero:Handle,
    one:Handle,
)->Option<u8>{
    let _=memory;
    if value==one{Some(1)}else if value==zero{Some(0)}else{None}
}

fn runtime_rotate_carry_word(
    memory:&ProofRuntimeMemory,
    word:Handle,
    zero:Handle,
    one:Handle,
)->Option<u32>{
    let bits=read_exact_sequence(&memory.store,word).ok()?;
    if bits.len()!=WIDTH{return None;}
    let mut value=0u32;
    for (index,bit) in bits.into_iter().enumerate(){
        value|=u32::from(runtime_rotate_carry_bit(memory,bit,zero,one)?)<<index;
    }
    Some(value)
}

fn runtime_rotate_carry_action(
    memory:&ProofRuntimeMemory,
    action:Handle,
    set_tag:Handle,
    undefined_tag:Handle,
    expected_flag:Handle,
    zero:Handle,
    one:Handle,
)->Option<FlagState>{
    let (tag,payload)=memory.store.poles(action).ok()?;
    let values=read_exact_sequence(&memory.store,payload).ok()?;
    if tag==set_tag{
        if values.len()!=2||values[0]!=expected_flag{return None;}
        Some(FlagState::Set(runtime_rotate_carry_bit(
            memory,values[1],zero,one,
        )?))
    }else if tag==undefined_tag{
        if values!=vec![expected_flag]{return None;}
        Some(FlagState::Undefined)
    }else{
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn decode_runtime_rotate_carry_effect(
    memory:&ProofRuntimeMemory,
    final_link:Handle,
    caller:Handle,
    result_tag:Handle,
    zero:Handle,
    one:Handle,
    set_tag:Handle,
    undefined_tag:Handle,
    cf_flag:Handle,
    of_flag:Handle,
    reactions:usize,
)->Option<(Outcome,Handle)>{
    let (final_caller,endpoint)=memory.store.poles(final_link).ok()?;
    if final_caller!=caller{return None;}
    let (tag,payload)=memory.store.poles(endpoint).ok()?;
    if tag!=result_tag{return None;}
    let values=read_exact_sequence(&memory.store,payload).ok()?;
    if values.len()!=3{return None;}

    let writeback=runtime_rotate_carry_bit(memory,values[0],zero,one)?;
    let value=runtime_rotate_carry_word(memory,values[1],zero,one)?;
    let patch=read_exact_sequence(&memory.store,values[2]).ok()?;
    let (cf,of,patch_len)=if patch.is_empty(){
        (FlagState::Preserve,FlagState::Preserve,0)
    }else{
        if patch.len()!=2{return None;}
        (
            runtime_rotate_carry_action(
                memory,patch[0],set_tag,undefined_tag,cf_flag,zero,one,
            )?,
            runtime_rotate_carry_action(
                memory,patch[1],set_tag,undefined_tag,of_flag,zero,one,
            )?,
            2,
        )
    };

    Some((Outcome{writeback,value,cf,of,patch_len,reactions},payload))
}


#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct RotateCarry32SessionProjection{
    pub(crate) value:u32,
    pub(crate) writeback:u8,
    pub(crate) defined_mask:u32,
    pub(crate) value_mask:u32,
    pub(crate) undefined_mask:u32,
    pub(crate) preserve_mask:u32,
    pub(crate) result_recursive_wire:String,
}

pub(crate) fn prepare_rotate_carry32_session_program(
)->Option<WebProofPrepareStage>{
    let mut compiler=FullFixture::new();
    let program=Program::install(&mut compiler);

    let prepared_roots=vec![
        semantic_source(&compiler.store,"function.rotate_carry.rcl",program.rcl),
        semantic_source(&compiler.store,"function.rotate_carry.rcr",program.rcr),
        semantic_source(&compiler.store,"function.gate.xor2",program.gates.xor2),
        semantic_source(&compiler.store,"data.bit.zero",compiler.zero),
        semantic_source(&compiler.store,"data.bit.one",compiler.one),
        semantic_source(&compiler.store,"execution.interpreter",compiler.interpreter),
        semantic_source(&compiler.store,"execution.theory",compiler.theory),
        semantic_source(&compiler.store,"execution.apply",compiler.apply),
        semantic_source(&compiler.store,"context.caller",compiler.k),
        semantic_source(&compiler.store,"result.tag",program.result_tag),
        semantic_source(&compiler.store,"result.flag.set_tag",program.schema.set_tag),
        semantic_source(
            &compiler.store,"result.flag.undefined_tag",program.schema.undefined_tag,
        ),
        semantic_source(&compiler.store,"result.flag.cf",program.schema.cf),
        semantic_source(&compiler.store,"result.flag.of",program.schema.of),
    ];
    let admissions=theory_admissions(&compiler.store,compiler.theory)?;
    Some(prepare_stage(&compiler.store,prepared_roots,admissions))
}

fn rotate_carry32_function_role(op:u32)->Option<&'static str>{
    match op{
        18=>Some("function.rotate_carry.rcl"),
        19=>Some("function.rotate_carry.rcr"),
        _=>None,
    }
}

pub(crate) fn configure_rotate_carry32_session(
    session:&mut ProofRuntimeSession,
    load:&WebProofLoadStage,
    op:u32,
    value:u32,
    count:u8,
    cf_in:u8,
)->Option<(Handle,usize,usize)>{
    if cf_in>1{return None;}
    let function=loaded_handle(load,rotate_carry32_function_role(op)?)?;
    let apply=loaded_handle(load,"execution.apply")?;
    let caller=loaded_handle(load,"context.caller")?;
    let zero=loaded_handle(load,"data.bit.zero")?;
    let one=loaded_handle(load,"data.bit.one")?;

    let word=|store:&mut OptimizedLinkStore,width:usize,raw:u32|{
        let bits=(0..width)
            .map(|bit|if (raw>>bit)&1==1{one}else{zero})
            .collect::<Vec<_>>();
        materialize_exact_sequence(store,&bits).ok()
    };

    let before=session.memory.store.link_count();
    let value_word=word(&mut session.memory.store,WIDTH,value)?;
    let count_word=word(&mut session.memory.store,8,u32::from(count))?;
    let cf_handle=if cf_in==0{zero}else{one};
    let args=materialize_exact_sequence(
        &mut session.memory.store,&[value_word,count_word,cf_handle],
    ).ok()?;
    let invocation=call(&mut session.memory.store,apply,function,args);
    let initial=session.memory.store.ensure_pair(caller,invocation).ok()?;
    let after=session.memory.store.link_count();

    Some((initial,before,after))
}

pub(crate) fn project_rotate_carry32_session_result(
    session:&ProofRuntimeSession,
    load:&WebProofLoadStage,
)->Option<RotateCarry32SessionProjection>{
    if session.engine.current().len()!=1{return None;}

    let caller=loaded_handle(load,"context.caller")?;
    let result_tag=loaded_handle(load,"result.tag")?;
    let zero=loaded_handle(load,"data.bit.zero")?;
    let one=loaded_handle(load,"data.bit.one")?;
    let set_tag=loaded_handle(load,"result.flag.set_tag")?;
    let undefined_tag=loaded_handle(load,"result.flag.undefined_tag")?;
    let cf_flag=loaded_handle(load,"result.flag.cf")?;
    let of_flag=loaded_handle(load,"result.flag.of")?;
    let final_link=session.engine.current()[0];

    let (actual,_)=decode_runtime_rotate_carry_effect(
        &session.memory,
        final_link,
        caller,
        result_tag,
        zero,
        one,
        set_tag,
        undefined_tag,
        cf_flag,
        of_flag,
        0,
    )?;

    let mut defined_mask=0u32;
    let mut value_mask=0u32;
    let mut undefined_mask=0u32;
    let mut preserve_mask=WEB_OTHER_STATUS;
    web_rotate_carry_state(
        WEB_CF,actual.cf,&mut defined_mask,&mut value_mask,
        &mut undefined_mask,&mut preserve_mask,
    );
    web_rotate_carry_state(
        WEB_OF,actual.of,&mut defined_mask,&mut value_mask,
        &mut undefined_mask,&mut preserve_mask,
    );

    Some(RotateCarry32SessionProjection{
        value:actual.value,
        writeback:actual.writeback,
        defined_mask,
        value_mask,
        undefined_mask,
        preserve_mask,
        result_recursive_wire:session.memory.store.export_anum(final_link).ok()?,
    })
}

pub(crate) fn web_prove_rotate_carry32(
    op:u32,
    value:u32,
    count:u32,
    cf_in:u32,
)->Option<WebRotateCarryProofExecution>{
    if count>u8::MAX as u32||cf_in>1{return None;}

    let mut compiler=FullFixture::new();
    let program=Program::install(&mut compiler);
    let (block,function)=match op{
        18=>("RCL32",program.rcl),
        19=>("RCR32",program.rcr),
        _=>return None,
    };
    let links_after_build=program.links_after_build as u32;

    let value_bits=bit_handles(&compiler,WIDTH,value);
    let count_bits=bit_handles(&compiler,8,count);
    let value_word=materialize_exact_sequence(&mut compiler.store,&value_bits).ok()?;
    let count_word=materialize_exact_sequence(&mut compiler.store,&count_bits).ok()?;
    let cf_handle=if cf_in==0{compiler.zero}else{compiler.one};
    let args=materialize_exact_sequence(
        &mut compiler.store,&[value_word,count_word,cf_handle],
    ).ok()?;
    let invocation=call(&mut compiler.store,compiler.apply,function,args);
    let initial=compiler.store.ensure_pair(compiler.k,invocation).ok()?;

    let prepared_roots=vec![
        semantic_source(&compiler.store,"function.rotate_carry.selected",function),
        semantic_source(&compiler.store,"function.rotate_carry.rcl",program.rcl),
        semantic_source(&compiler.store,"function.rotate_carry.rcr",program.rcr),
        semantic_source(&compiler.store,"function.gate.xor2",program.gates.xor2),
        semantic_source(&compiler.store,"data.value.word",value_word),
        semantic_source(&compiler.store,"data.count.word",count_word),
        semantic_source(&compiler.store,"data.cf_in",cf_handle),
        semantic_source(&compiler.store,"data.bit.zero",compiler.zero),
        semantic_source(&compiler.store,"data.bit.one",compiler.one),
        semantic_source(&compiler.store,"execution.interpreter",compiler.interpreter),
        semantic_source(&compiler.store,"execution.theory",compiler.theory),
        semantic_source(&compiler.store,"execution.apply",compiler.apply),
        semantic_source(&compiler.store,"invocation.args",args),
        semantic_source(&compiler.store,"invocation.call",invocation),
        semantic_source(&compiler.store,"scope.initial",initial),
        semantic_source(&compiler.store,"context.caller",compiler.k),
        semantic_source(&compiler.store,"result.tag",program.result_tag),
        semantic_source(&compiler.store,"result.flag.set_tag",program.schema.set_tag),
        semantic_source(
            &compiler.store,"result.flag.undefined_tag",program.schema.undefined_tag,
        ),
        semantic_source(&compiler.store,"result.flag.cf",program.schema.cf),
        semantic_source(&compiler.store,"result.flag.of",program.schema.of),
    ];

    let admissions=theory_admissions(&compiler.store,compiler.theory)?;
    let prepare=prepare_stage(&compiler.store,prepared_roots,admissions);
    let (mut memory,load)=load_runtime(&prepare)?;

    let interpreter=loaded_handle(&load,"execution.interpreter")?;
    let initial=loaded_handle(&load,"scope.initial")?;
    let caller=loaded_handle(&load,"context.caller")?;
    let result_tag=loaded_handle(&load,"result.tag")?;
    let zero=loaded_handle(&load,"data.bit.zero")?;
    let one=loaded_handle(&load,"data.bit.one")?;
    let set_tag=loaded_handle(&load,"result.flag.set_tag")?;
    let undefined_tag=loaded_handle(&load,"result.flag.undefined_tag")?;
    let cf_flag=loaded_handle(&load,"result.flag.cf")?;
    let of_flag=loaded_handle(&load,"result.flag.of")?;

    let (mut engine,execute)=execute_to_quiescence(
        &mut memory,interpreter,initial,32,6,
    )?;
    if engine.current().len()!=1{return None;}

    let final_link=engine.current()[0];
    let (actual,payload)=decode_runtime_rotate_carry_effect(
        &memory,
        final_link,
        caller,
        result_tag,
        zero,
        one,
        set_tag,
        undefined_tag,
        cf_flag,
        of_flag,
        execute.active_reaction_count as usize,
    )?;
    let oracle=expected(&program,function,value,count as u8,cf_in as u8);
    if execute.active_reaction_count!=actual.reactions as u32{return None;}

    let result_anum=memory.store.export_anum(final_link).ok()?;
    let result_sequence_anum=memory.store.export_anum(payload).ok()?;
    let links_after_first=memory.store.link_count() as u32;
    let identical_rerun_link_delta=identical_rerun(
        &mut memory,&mut engine,initial,&result_anum,6,
    )?;
    let visual_links=visual_snapshot(&memory,&load.semantic_roots);

    let result=WebProofResultStage{
        memory_instance_id:memory.id.clone(),
        result_anum,
        result_sequence_anum,
        decoded_value:actual.value,
        decoded_value_hi:None,
        oracle_value:oracle.value,
        oracle_value_hi:None,
        oracle_matches:actual==oracle,
        links_final:memory.store.link_count() as u32,
        identical_rerun_link_delta,
        visual_links,
    };
    let proof=WebStructuralProof{
        schema_version: 4,
        block:block.to_owned(),
        prepare,
        load,
        execute,
        result,
    };

    let mut defined_mask=0u32;
    let mut value_mask=0u32;
    let mut undefined_mask=0u32;
    let mut preserve_mask=WEB_OTHER_STATUS;
    web_rotate_carry_state(
        WEB_CF,actual.cf,&mut defined_mask,&mut value_mask,
        &mut undefined_mask,&mut preserve_mask,
    );
    web_rotate_carry_state(
        WEB_OF,actual.of,&mut defined_mask,&mut value_mask,
        &mut undefined_mask,&mut preserve_mask,
    );

    let outcome=WebRotateCarryOutcome{
        value:actual.value,
        writeback:actual.writeback,
        defined_mask,
        value_mask,
        undefined_mask,
        preserve_mask,
        reactions:actual.reactions as u32,
        links_after_build,
        links_after_first,
        steady_link_delta:proof.result.identical_rerun_link_delta,
        quiescent:u8::from(proof.execute.final_quiescent),
    };

    Some(WebRotateCarryProofExecution{outcome,proof})
}

pub(crate) fn web_run_rotate_carry32(
    op:u32,
    value:u32,
    count:u32,
    cf_in:u32,
)->Option<WebRotateCarryOutcome>{
    if count>u8::MAX as u32 || cf_in>1{return None;}

    let mut f=FullFixture::new();
    let p=Program::install(&mut f);
    let function=match op{18=>p.rcl,19=>p.rcr,_=>return None};
    let links_after_build=f.store.link_count() as u32;

    let first=run(&mut f,&p,function,value,count as u8,cf_in as u8);
    let links_after_first=f.store.link_count() as u32;
    let second=run(&mut f,&p,function,value,count as u8,cf_in as u8);
    assert_eq!(second,first,"web RCL/RCR repeat changed result");
    let links_after_second=f.store.link_count() as u32;

    let mut defined_mask=0u32;
    let mut value_mask=0u32;
    let mut undefined_mask=0u32;
    let mut preserve_mask=WEB_OTHER_STATUS;
    web_rotate_carry_state(WEB_CF,first.cf,&mut defined_mask,&mut value_mask,&mut undefined_mask,&mut preserve_mask);
    web_rotate_carry_state(WEB_OF,first.of,&mut defined_mask,&mut value_mask,&mut undefined_mask,&mut preserve_mask);

    Some(WebRotateCarryOutcome{
        value:first.value,
        writeback:first.writeback,
        defined_mask,
        value_mask,
        undefined_mask,
        preserve_mask,
        reactions:first.reactions as u32,
        links_after_build,
        links_after_first,
        steady_link_delta:links_after_second-links_after_first,
        quiescent:1,
    })
}


#[test]
fn persistent_rotate_carry32_session_lifecycle_all_ops(){
    let run_vectors=[
        (0x8000_0001u32,32u8,0u8),
        (0x8000_0001u32,33u8,1u8),
        (0x8000_0001u32,2u8,0u8),
        (0x8000_0001u32,1u8,0u8),
        (0x8000_0001u32,32u8,0u8),
    ];

    for (op,kind) in [(18u32,Kind::Rcl),(19u32,Kind::Rcr)]{
        let prepare=prepare_rotate_carry32_session_program()
            .expect("prepare ROTATE-CARRY32");
        let prepared_roles=prepare.semantic_roots.iter()
            .map(|root|root.role.as_str())
            .collect::<std::collections::BTreeSet<_>>();

        for runtime_role in [
            "function.rotate_carry.selected",
            "data.value.word",
            "data.count.word",
            "data.cf_in",
            "invocation.args",
            "invocation.call",
            "scope.initial",
        ]{
            assert!(
                !prepared_roles.contains(runtime_role),
                "op {op}: PREPARE leaked runtime role {runtime_role}",
            );
        }
        for static_role in [
            "function.rotate_carry.rcl",
            "function.rotate_carry.rcr",
            "function.gate.xor2",
        ]{
            assert!(
                prepared_roles.contains(static_role),
                "op {op}: static ROTATE-CARRY32 Aset missing {static_role}",
            );
        }

        let (mut session,load)=load_runtime_session(&prepare,64)
            .expect("load ROTATE-CARRY32");
        let memory_id=session.memory.id.clone();
        let base_link_count=session.base_link_count;
        let base_carrier=session.memory.store.export_packed_duplets();

        let mut oracle_fixture=FullFixture::new();
        let oracle_program=Program::install(&mut oracle_fixture);
        let oracle_function=match kind{
            Kind::Rcl=>oracle_program.rcl,
            Kind::Rcr=>oracle_program.rcr,
        };

        let mut first_projection=None;
        for (index,(value,count,cf_in)) in
            run_vectors.into_iter().enumerate()
        {
            let (initial,before,after)=configure_rotate_carry32_session(
                &mut session,&load,op,value,count,cf_in,
            ).expect("configure ROTATE-CARRY32");
            assert_eq!(session.memory.id,memory_id);
            assert!(before>=base_link_count);
            assert!(after>=before);

            if index==4{
                assert_eq!(
                    after,before,
                    "op {op}: return-to-first must reuse canonical configuration Links",
                );
            }

            let execute=execute_session_to_quiescence(
                &mut session,initial,16,
            ).expect("execute ROTATE-CARRY32");
            assert!(execute.final_quiescent);
            assert_eq!(session.memory.id,memory_id);
            assert_eq!(session.engine.current().len(),1);

            let projected=project_rotate_carry32_session_result(
                &session,&load,
            ).expect("project ROTATE-CARRY32");
            let expected=expected(
                &oracle_program,oracle_function,value,count,cf_in,
            );

            let mut expected_defined=0u32;
            let mut expected_values=0u32;
            let mut expected_undefined=0u32;
            let mut expected_preserve=WEB_OTHER_STATUS;
            web_rotate_carry_state(
                WEB_CF,expected.cf,&mut expected_defined,&mut expected_values,
                &mut expected_undefined,&mut expected_preserve,
            );
            web_rotate_carry_state(
                WEB_OF,expected.of,&mut expected_defined,&mut expected_values,
                &mut expected_undefined,&mut expected_preserve,
            );

            assert_eq!(
                execute.active_reaction_count,expected.reactions as u32,
                "op {op} run {index}: reaction count",
            );
            assert_eq!(projected.value,expected.value);
            assert_eq!(projected.writeback,expected.writeback);
            assert_eq!(projected.defined_mask,expected_defined);
            assert_eq!(projected.value_mask,expected_values);
            assert_eq!(projected.undefined_mask,expected_undefined);
            assert_eq!(projected.preserve_mask,expected_preserve);

            let carrier=session.memory.store.export_packed_duplets();
            assert_eq!(
                &carrier[..base_link_count],base_carrier.as_slice(),
                "op {op}: loaded base prefix changed",
            );

            if index==0{
                first_projection=Some(projected.clone());
            }else if index==4{
                assert_eq!(
                    Some(projected),first_projection,
                    "op {op}: return-to-first changed semantic Result",
                );
            }
        }
    }
}

#[test]
#[ignore="heavy M4 RCL/RCR suite; mandatory release workflow"]
fn m4_rotate_carry32_matches_80386_33bit_ring(){
    let mut f=FullFixture::new(); let p=Program::install(&mut f);
    let sentinels=[0u32,0x8000_0001,0xa5a5_5a5a];
    for count in counts(){
        for value in sentinels{
            for cf in [0u8,1]{
                for function in [p.rcl,p.rcr]{
                    let actual=run(&mut f,&p,function,value,count,cf);
                    assert_eq!(actual,expected(&p,function,value,count,cf),
                        "value={value:#010x} count={count} cf={cf} function={function}");
                }
            }
        }
    }
    for count in [0u8,1,2,31,33]{
        for value in values(){
            for cf in [0u8,1]{
                for function in [p.rcl,p.rcr]{
                    assert_eq!(run(&mut f,&p,function,value,count,cf),
                        expected(&p,function,value,count,cf));
                }
            }
        }
    }
    println!("M4_ROTATE_CARRY32 rules={} program_links={}",2*32,p.links_after_build);
}

#[test]
#[ignore="heavy M4 RCL/RCR suite; mandatory release workflow"]
fn m4_rotate_carry32_aliases_cf_input_and_steady(){
    let mut f=FullFixture::new(); let p=Program::install(&mut f);
    let value=0x9234_5679u32;
    for function in [p.rcl,p.rcr]{
        for cf in [0u8,1]{
            let a=run(&mut f,&p,function,value,0,cf);
            assert_eq!(a,run(&mut f,&p,function,value,32,cf));
            assert_eq!(a,run(&mut f,&p,function,value,64,cf));

            let a=run(&mut f,&p,function,value,1,cf);
            assert_eq!(a,run(&mut f,&p,function,value,33,cf));
            assert_eq!(a,run(&mut f,&p,function,value,65,cf));

            let a=run(&mut f,&p,function,value,31,cf);
            assert_eq!(a,run(&mut f,&p,function,value,63,cf));
            assert_eq!(a,run(&mut f,&p,function,value,95,cf));
        }
    }

    let first=run(&mut f,&p,p.rcr,value,255,1);
    let links=f.store.link_count();
    let second=run(&mut f,&p,p.rcr,value,255,1);
    assert_eq!(first,second);
    assert_eq!(f.store.link_count(),links,"repeated identical RCR32 materialized Links");
}
