use super::{
    flag_patch::{
        install_alu_effect_result_tag, set_flag_action, FlagPatchSchema,
    },
    flags_n::FlaggedArithmeticProgram,
    full_adder::{
        call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
    },
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
    Handle, OptimizedLinkStore,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UnaryKind { Inc, Dec, Neg }

struct AnchorGen {
    current: Handle,
    flip: bool,
    o: Handle,
    c: Handle,
}
impl AnchorGen {
    fn new(
        store: &mut OptimizedLinkStore,
        mut seed: Handle,
        o: Handle,
        c: Handle,
        width: usize,
    ) -> Self {
        for pole in [o,c,c,c,o,c,o,o,c] {
            seed=store.ensure_pair(seed,pole).unwrap();
        }
        for _ in 0..width {
            seed=store.ensure_pair(seed,o).unwrap();
            seed=store.ensure_pair(seed,c).unwrap();
        }
        Self{current:seed,flip:false,o,c}
    }
    fn next(&mut self,store:&mut OptimizedLinkStore)->Handle{
        let pole=if self.flip {self.c}else{self.o};
        self.flip=!self.flip;
        self.current=store.ensure_pair(self.current,pole).unwrap();
        self.current
    }
}

fn stage_frame(
    store:&mut OptimizedLinkStore,
    tag:Handle,
    values:&[Handle],
)->Handle{
    let payload=materialize_exact_sequence(store,values).unwrap();
    let descriptor=store.ensure_pair(tag,payload).unwrap();
    store.ensure_start_self_closed(descriptor).unwrap()
}

#[derive(Clone,Debug)]
struct Program{
    width:usize,
    inc:Handle,
    dec:Handle,
    neg:Handle,
    result_tag:Handle,
    schema:FlagPatchSchema,
    flagged:FlaggedArithmeticProgram,
    active_steps:usize,
    links_after_build:usize,
}

fn install_open(
    f:&mut FullFixture,
    a:&mut AnchorGen,
    function:Handle,
    flagged:Handle,
    continuation_tag:Handle,
    first:Option<Handle>,
    second:Option<Handle>,
    x:Handle,
    mode:Handle,
){
    let k=a.next(&mut f.store);
    let operand=a.next(&mut f.store);

    let input_args=materialize_exact_sequence(&mut f.store,&[operand]).unwrap();
    let before_call=call(&mut f.store,f.apply,function,input_args);
    let before=f.store.ensure_pair(k,before_call).unwrap();

    assert_ne!(first.is_none(), second.is_none(),
        "exactly one arithmetic operand must be the runtime unary role");
    let lhs=first.unwrap_or(operand);
    let rhs=second.unwrap_or(operand);

    let flagged_args=
        materialize_exact_sequence(&mut f.store,&[lhs,rhs,x,mode]).unwrap();
    let flagged_call=call(&mut f.store,f.apply,flagged,flagged_args);
    let caller=stage_frame(&mut f.store,continuation_tag,&[k]);
    let after=f.store.ensure_pair(caller,flagged_call).unwrap();

    let (_,admission)=define_bundle_rule(
        &mut f.store,
        f.theory,
        &[k,operand],
        before,
        &[after],
    );
    index_rule_for(&mut f.store,&[f.o],admission);
}

impl Program{
    fn install(f:&mut FullFixture,width:usize)->Self{
        assert!((8..=32).contains(&width));
        let flagged=FlaggedArithmeticProgram::install(f,width);

        let seed=f.store.ensure_pair(flagged.flagged,flagged.result_tag).unwrap();
        let mut a=AnchorGen::new(&mut f.store,seed,f.o,f.c,width);

        let l=a.next(&mut f.store); let r=a.next(&mut f.store);
        let inc=f.store.ensure_pair(l,r).unwrap();
        let l=a.next(&mut f.store); let r=a.next(&mut f.store);
        let dec=f.store.ensure_pair(l,r).unwrap();
        let l=a.next(&mut f.store); let r=a.next(&mut f.store);
        let neg=f.store.ensure_pair(l,r).unwrap();

        let incdec_tag=a.next(&mut f.store);
        let neg_tag=a.next(&mut f.store);

        let zeros=vec![f.zero;width];
        let zero_word=materialize_exact_sequence(&mut f.store,&zeros).unwrap();
        let mut ones=zeros;
        ones[0]=f.one;
        let one_word=materialize_exact_sequence(&mut f.store,&ones).unwrap();

        install_open(f,&mut a,inc,flagged.flagged,incdec_tag,
            None,Some(one_word),f.zero,f.zero);
        install_open(f,&mut a,dec,flagged.flagged,incdec_tag,
            None,Some(one_word),f.zero,f.one);
        install_open(f,&mut a,neg,flagged.flagged,neg_tag,
            Some(zero_word),None,f.zero,f.one);

        let shared=f.store.ensure_pair(f.k,f.full).unwrap();
        let schema=FlagPatchSchema::install(&mut f.store,shared,f.o,f.c);
        let result_tag=install_alu_effect_result_tag(&mut f.store,shared,f.o,f.c);

        // INC/DEC: consume the complete flagged arithmetic result but omit CF
        // from the patch. Absence means PRESERVE.
        {
            let k=a.next(&mut f.store);
            let word=a.next(&mut f.store);
            let cf=a.next(&mut f.store);
            let pf=a.next(&mut f.store);
            let af=a.next(&mut f.store);
            let zf=a.next(&mut f.store);
            let sf=a.next(&mut f.store);
            let of=a.next(&mut f.store);

            let flagged_payload=materialize_exact_sequence(
                &mut f.store,&[word,cf,pf,af,zf,sf,of]).unwrap();
            let flagged_envelope=
                f.store.ensure_pair(flagged.result_tag,flagged_payload).unwrap();
            let caller=stage_frame(&mut f.store,incdec_tag,&[k]);
            let before=f.store.ensure_pair(caller,flagged_envelope).unwrap();

            let actions=[
                set_flag_action(&mut f.store,schema,schema.pf,pf),
                set_flag_action(&mut f.store,schema,schema.af,af),
                set_flag_action(&mut f.store,schema,schema.zf,zf),
                set_flag_action(&mut f.store,schema,schema.sf,sf),
                set_flag_action(&mut f.store,schema,schema.of,of),
            ];
            let patch=materialize_exact_sequence(&mut f.store,&actions).unwrap();
            let payload=materialize_exact_sequence(&mut f.store,&[f.one,word,patch]).unwrap();
            let envelope=f.store.ensure_pair(result_tag,payload).unwrap();
            let after=f.store.ensure_pair(k,envelope).unwrap();

            let roles=[k,word,cf,pf,af,zf,sf,of];
            let (_,admission)=define_bundle_rule(
                &mut f.store,f.theory,&roles,before,&[after]);
            index_rule_for(&mut f.store,&[flagged.result_tag],admission);
        }

        // NEG: publish the complete arithmetic patch, including CF.
        {
            let k=a.next(&mut f.store);
            let word=a.next(&mut f.store);
            let cf=a.next(&mut f.store);
            let pf=a.next(&mut f.store);
            let af=a.next(&mut f.store);
            let zf=a.next(&mut f.store);
            let sf=a.next(&mut f.store);
            let of=a.next(&mut f.store);

            let flagged_payload=materialize_exact_sequence(
                &mut f.store,&[word,cf,pf,af,zf,sf,of]).unwrap();
            let flagged_envelope=
                f.store.ensure_pair(flagged.result_tag,flagged_payload).unwrap();
            let caller=stage_frame(&mut f.store,neg_tag,&[k]);
            let before=f.store.ensure_pair(caller,flagged_envelope).unwrap();

            let actions=[
                set_flag_action(&mut f.store,schema,schema.cf,cf),
                set_flag_action(&mut f.store,schema,schema.pf,pf),
                set_flag_action(&mut f.store,schema,schema.af,af),
                set_flag_action(&mut f.store,schema,schema.zf,zf),
                set_flag_action(&mut f.store,schema,schema.sf,sf),
                set_flag_action(&mut f.store,schema,schema.of,of),
            ];
            let patch=materialize_exact_sequence(&mut f.store,&actions).unwrap();
            let payload=materialize_exact_sequence(&mut f.store,&[f.one,word,patch]).unwrap();
            let envelope=f.store.ensure_pair(result_tag,payload).unwrap();
            let after=f.store.ensure_pair(k,envelope).unwrap();

            let roles=[k,word,cf,pf,af,zf,sf,of];
            let (_,admission)=define_bundle_rule(
                &mut f.store,f.theory,&roles,before,&[after]);
            index_rule_for(&mut f.store,&[flagged.result_tag],admission);
        }

        Self{
            width,inc,dec,neg,result_tag,schema,
            active_steps:flagged.active_steps+2,
            flagged,
            links_after_build:f.store.link_count(),
        }
    }
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
struct Outcome{
    value:u32,
    cf:Option<u8>,
    pf:u8,
    af:u8,
    zf:u8,
    sf:u8,
    of:u8,
    patch_len:usize,
}
fn mask(width:usize)->u32{
    if width==32 {u32::MAX}else{(1u32<<width)-1}
}
fn bit_handles(f:&FullFixture,width:usize,value:u32)->Vec<Handle>{
    (0..width).map(|i|if (value>>i)&1==1{f.one}else{f.zero}).collect()
}
fn decode_bit(f:&FullFixture,h:Handle)->u8{
    if h==f.one {1}else{assert_eq!(h,f.zero);0}
}
fn decode_word(f:&FullFixture,width:usize,h:Handle)->u32{
    let bits=read_exact_sequence(&f.store,h).unwrap(); assert_eq!(bits.len(),width);
    let mut v=0;
    for(i,b)in bits.into_iter().enumerate(){v|=u32::from(decode_bit(f,b))<<i;}
    v
}
fn decode_set(f:&FullFixture,s:FlagPatchSchema,a:Handle,flag:Handle)->u8{
    let(tag,payload)=f.store.poles(a).unwrap(); assert_eq!(tag,s.set_tag);
    let vals=read_exact_sequence(&f.store,payload).unwrap();
    assert_eq!(vals.len(),2); assert_eq!(vals[0],flag); decode_bit(f,vals[1])
}
fn decode(f:&FullFixture,p:&Program)->Outcome{
    assert_eq!(f.engine.current().len(),1);
    let(caller,endpoint)=f.store.poles(f.engine.current()[0]).unwrap();
    assert_eq!(caller,f.k);
    let(tag,payload)=f.store.poles(endpoint).unwrap(); assert_eq!(tag,p.result_tag);
    let vals=read_exact_sequence(&f.store,payload).unwrap(); assert_eq!(vals.len(),3);
    assert_eq!(decode_bit(f,vals[0]),1);
    let value=decode_word(f,p.width,vals[1]);
    let patch=read_exact_sequence(&f.store,vals[2]).unwrap();
    let s=p.schema;
    if patch.len()==5 {
        Outcome{
            value,cf:None,
            pf:decode_set(f,s,patch[0],s.pf),
            af:decode_set(f,s,patch[1],s.af),
            zf:decode_set(f,s,patch[2],s.zf),
            sf:decode_set(f,s,patch[3],s.sf),
            of:decode_set(f,s,patch[4],s.of),
            patch_len:5,
        }
    } else {
        assert_eq!(patch.len(),6);
        Outcome{
            value,cf:Some(decode_set(f,s,patch[0],s.cf)),
            pf:decode_set(f,s,patch[1],s.pf),
            af:decode_set(f,s,patch[2],s.af),
            zf:decode_set(f,s,patch[3],s.zf),
            sf:decode_set(f,s,patch[4],s.sf),
            of:decode_set(f,s,patch[5],s.of),
            patch_len:6,
        }
    }
}
fn run(f:&mut FullFixture,p:&Program,function:Handle,value:u32)->Outcome{
    let m=mask(p.width); assert_eq!(value&!m,0);
    let bits=bit_handles(f,p.width,value);
    let word=materialize_exact_sequence(&mut f.store,&bits).unwrap();
    let args=materialize_exact_sequence(&mut f.store,&[word]).unwrap();
    let inv=call(&mut f.store,f.apply,function,args);
    let initial=f.store.ensure_pair(f.k,inv).unwrap();
    f.engine.set_current(&f.store,&[initial]).unwrap();

    for step in 0..p.active_steps {
        let x=f.engine.run(&mut f.store).unwrap();
        assert!(!x.quiescent,"step {step}");
        assert_eq!(x.raw_rule_matches,1,"step {step}");
        assert_eq!(x.transitioned_members,1,"step {step}");
        assert_eq!(x.handoff_count,1,"step {step}");
        assert_eq!(x.next_members.len(),1,"step {step}");
    }
    let stable=f.engine.current_bank();
    let q=f.engine.run(&mut f.store).unwrap();
    assert!(q.quiescent); assert_eq!(q.raw_rule_matches,0);
    assert_eq!(q.handoff_count,0); assert_eq!(f.engine.current_bank(),stable);
    decode(f,p)
}
fn expected(width:usize,kind:UnaryKind,a:u32)->Outcome{
    let m=mask(width);
    let sign=1u32<<(width-1);
    let max_pos=sign-1;
    let value=match kind {
        UnaryKind::Inc=>a.wrapping_add(1)&m,
        UnaryKind::Dec=>a.wrapping_sub(1)&m,
        UnaryKind::Neg=>(0u32).wrapping_sub(a)&m,
    };
    let af=match kind {
        UnaryKind::Inc=>u8::from((a&0xf)==0xf),
        UnaryKind::Dec=>u8::from((a&0xf)==0),
        UnaryKind::Neg=>u8::from((a&0xf)!=0),
    };
    let of=match kind {
        UnaryKind::Inc=>u8::from(a==max_pos),
        UnaryKind::Dec=>u8::from(a==sign),
        UnaryKind::Neg=>u8::from(a==sign),
    };
    Outcome{
        value,
        cf:if kind==UnaryKind::Neg{Some(u8::from(a!=0))}else{None},
        pf:u8::from((value as u8).count_ones()%2==0),
        af,
        zf:u8::from(value==0),
        sf:((value>>(width-1))&1)as u8,
        of,
        patch_len:if kind==UnaryKind::Neg{6}else{5},
    }
}
fn vectors(width:usize)->Vec<u32>{
    let m=mask(width); let sign=1u32<<(width-1); let max_pos=sign-1;
    let mut v=vec![
        0,1,0xf&m,0x10&m,max_pos,sign,m,
        0x7f&m,0x80&m,0xaaaa_aaaa&m,0x5555_5555&m,
    ];
    let mut z=0x7f4a_7c15u32^width as u32;
    for _ in 0..6{z=z.wrapping_mul(1664525).wrapping_add(1013904223);v.push(z&m);}
    v.sort_unstable();v.dedup();v
}


#[derive(Clone,Debug)]
pub(crate) struct WebUnaryProofExecution{
    pub(crate) outcome:WebUnaryOutcome,
    pub(crate) proof:WebStructuralProof,
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub(crate) struct WebUnaryOutcome{
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

fn runtime_unary_bit(
    memory:&ProofRuntimeMemory,
    value:Handle,
    zero:Handle,
    one:Handle,
)->Option<u8>{
    let _=memory;
    if value==one{Some(1)}else if value==zero{Some(0)}else{None}
}

fn runtime_unary_word(
    memory:&ProofRuntimeMemory,
    word:Handle,
    zero:Handle,
    one:Handle,
)->Option<u32>{
    let bits=read_exact_sequence(&memory.store,word).ok()?;
    if bits.len()!=32{return None;}
    let mut value=0u32;
    for (index,bit) in bits.into_iter().enumerate(){
        value|=u32::from(runtime_unary_bit(memory,bit,zero,one)?)<<index;
    }
    Some(value)
}

fn runtime_unary_set(
    memory:&ProofRuntimeMemory,
    action:Handle,
    set_tag:Handle,
    expected_flag:Handle,
    zero:Handle,
    one:Handle,
)->Option<u8>{
    let (tag,payload)=memory.store.poles(action).ok()?;
    if tag!=set_tag{return None;}
    let values=read_exact_sequence(&memory.store,payload).ok()?;
    if values.len()!=2||values[0]!=expected_flag{return None;}
    runtime_unary_bit(memory,values[1],zero,one)
}

#[allow(clippy::too_many_arguments)]
fn decode_runtime_unary_effect(
    memory:&ProofRuntimeMemory,
    final_link:Handle,
    caller:Handle,
    result_tag:Handle,
    zero:Handle,
    one:Handle,
    set_tag:Handle,
    cf_flag:Handle,
    pf_flag:Handle,
    af_flag:Handle,
    zf_flag:Handle,
    sf_flag:Handle,
    of_flag:Handle,
)->Option<(Outcome,Handle)>{
    let (final_caller,endpoint)=memory.store.poles(final_link).ok()?;
    if final_caller!=caller{return None;}
    let (tag,payload)=memory.store.poles(endpoint).ok()?;
    if tag!=result_tag{return None;}
    let values=read_exact_sequence(&memory.store,payload).ok()?;
    if values.len()!=3{return None;}
    if runtime_unary_bit(memory,values[0],zero,one)?!=1{return None;}

    let value=runtime_unary_word(memory,values[1],zero,one)?;
    let patch=read_exact_sequence(&memory.store,values[2]).ok()?;

    let outcome=if patch.len()==5{
        Outcome{
            value,
            cf:None,
            pf:runtime_unary_set(memory,patch[0],set_tag,pf_flag,zero,one)?,
            af:runtime_unary_set(memory,patch[1],set_tag,af_flag,zero,one)?,
            zf:runtime_unary_set(memory,patch[2],set_tag,zf_flag,zero,one)?,
            sf:runtime_unary_set(memory,patch[3],set_tag,sf_flag,zero,one)?,
            of:runtime_unary_set(memory,patch[4],set_tag,of_flag,zero,one)?,
            patch_len:5,
        }
    }else{
        if patch.len()!=6{return None;}
        Outcome{
            value,
            cf:Some(runtime_unary_set(
                memory,patch[0],set_tag,cf_flag,zero,one,
            )?),
            pf:runtime_unary_set(memory,patch[1],set_tag,pf_flag,zero,one)?,
            af:runtime_unary_set(memory,patch[2],set_tag,af_flag,zero,one)?,
            zf:runtime_unary_set(memory,patch[3],set_tag,zf_flag,zero,one)?,
            sf:runtime_unary_set(memory,patch[4],set_tag,sf_flag,zero,one)?,
            of:runtime_unary_set(memory,patch[5],set_tag,of_flag,zero,one)?,
            patch_len:6,
        }
    };

    Some((outcome,payload))
}


#[derive(Clone,Debug,PartialEq,Eq)]
pub(crate) struct Unary32SessionProjection{
    pub(crate) value:u32,
    pub(crate) writeback:u8,
    pub(crate) defined_mask:u32,
    pub(crate) value_mask:u32,
    pub(crate) undefined_mask:u32,
    pub(crate) preserve_mask:u32,
    pub(crate) result_recursive_wire:String,
}

pub(crate) fn prepare_unary32_session_program(
)->Option<WebProofPrepareStage>{
    let mut compiler=FullFixture::new();
    let program=Program::install(&mut compiler,32);

    let prepared_roots=vec![
        semantic_source(&compiler.store,"function.unary.inc",program.inc),
        semantic_source(&compiler.store,"function.unary.dec",program.dec),
        semantic_source(&compiler.store,"function.unary.neg",program.neg),
        semantic_source(
            &compiler.store,"function.flagged_arithmetic",program.flagged.flagged,
        ),
        semantic_source(
            &compiler.store,"result.flagged_tag",program.flagged.result_tag,
        ),
        semantic_source(&compiler.store,"data.bit.zero",compiler.zero),
        semantic_source(&compiler.store,"data.bit.one",compiler.one),
        semantic_source(&compiler.store,"execution.interpreter",compiler.interpreter),
        semantic_source(&compiler.store,"execution.theory",compiler.theory),
        semantic_source(&compiler.store,"execution.apply",compiler.apply),
        semantic_source(&compiler.store,"context.caller",compiler.k),
        semantic_source(&compiler.store,"result.tag",program.result_tag),
        semantic_source(&compiler.store,"result.flag.set_tag",program.schema.set_tag),
        semantic_source(&compiler.store,"result.flag.cf",program.schema.cf),
        semantic_source(&compiler.store,"result.flag.pf",program.schema.pf),
        semantic_source(&compiler.store,"result.flag.af",program.schema.af),
        semantic_source(&compiler.store,"result.flag.zf",program.schema.zf),
        semantic_source(&compiler.store,"result.flag.sf",program.schema.sf),
        semantic_source(&compiler.store,"result.flag.of",program.schema.of),
    ];
    let admissions=theory_admissions(&compiler.store,compiler.theory)?;
    Some(prepare_stage(&compiler.store,prepared_roots,admissions))
}

fn unary32_function_role(op:u32)->Option<&'static str>{
    match op{
        20=>Some("function.unary.inc"),
        21=>Some("function.unary.dec"),
        22=>Some("function.unary.neg"),
        _=>None,
    }
}

pub(crate) fn configure_unary32_session(
    session:&mut ProofRuntimeSession,
    load:&WebProofLoadStage,
    op:u32,
    value:u32,
)->Option<(Handle,usize,usize)>{
    let function=loaded_handle(load,unary32_function_role(op)?)?;
    let apply=loaded_handle(load,"execution.apply")?;
    let caller=loaded_handle(load,"context.caller")?;
    let zero=loaded_handle(load,"data.bit.zero")?;
    let one=loaded_handle(load,"data.bit.one")?;

    let bits=(0..32)
        .map(|bit|if (value>>bit)&1==1{one}else{zero})
        .collect::<Vec<_>>();

    let before=session.memory.store.link_count();
    let word=materialize_exact_sequence(&mut session.memory.store,&bits).ok()?;
    let args=materialize_exact_sequence(&mut session.memory.store,&[word]).ok()?;
    let invocation=call(&mut session.memory.store,apply,function,args);
    let initial=session.memory.store.ensure_pair(caller,invocation).ok()?;
    let after=session.memory.store.link_count();

    Some((initial,before,after))
}

pub(crate) fn project_unary32_session_result(
    session:&ProofRuntimeSession,
    load:&WebProofLoadStage,
)->Option<Unary32SessionProjection>{
    if session.engine.current().len()!=1{return None;}

    let caller=loaded_handle(load,"context.caller")?;
    let result_tag=loaded_handle(load,"result.tag")?;
    let zero=loaded_handle(load,"data.bit.zero")?;
    let one=loaded_handle(load,"data.bit.one")?;
    let set_tag=loaded_handle(load,"result.flag.set_tag")?;
    let cf_flag=loaded_handle(load,"result.flag.cf")?;
    let pf_flag=loaded_handle(load,"result.flag.pf")?;
    let af_flag=loaded_handle(load,"result.flag.af")?;
    let zf_flag=loaded_handle(load,"result.flag.zf")?;
    let sf_flag=loaded_handle(load,"result.flag.sf")?;
    let of_flag=loaded_handle(load,"result.flag.of")?;
    let final_link=session.engine.current()[0];

    let (actual,_)=decode_runtime_unary_effect(
        &session.memory,
        final_link,
        caller,
        result_tag,
        zero,
        one,
        set_tag,
        cf_flag,
        pf_flag,
        af_flag,
        zf_flag,
        sf_flag,
        of_flag,
    )?;

    let mut defined_mask=WEB_PF|WEB_AF|WEB_ZF|WEB_SF|WEB_OF;
    let mut value_mask=0u32;
    for (flag_mask,bit) in [
        (WEB_PF,actual.pf),
        (WEB_AF,actual.af),
        (WEB_ZF,actual.zf),
        (WEB_SF,actual.sf),
        (WEB_OF,actual.of),
    ]{
        if bit!=0{value_mask|=flag_mask;}
    }
    let preserve_mask=if let Some(cf)=actual.cf{
        defined_mask|=WEB_CF;
        if cf!=0{value_mask|=WEB_CF;}
        0
    }else{
        WEB_CF
    };

    Some(Unary32SessionProjection{
        value:actual.value,
        writeback:1,
        defined_mask,
        value_mask,
        undefined_mask:0,
        preserve_mask,
        result_recursive_wire:session.memory.store.export_anum(final_link).ok()?,
    })
}

pub(crate) fn web_prove_unary32(
    op:u32,
    value:u32,
)->Option<WebUnaryProofExecution>{
    let mut compiler=FullFixture::new();
    let program=Program::install(&mut compiler,32);
    let (block,kind,function)=match op{
        20=>("INC32",UnaryKind::Inc,program.inc),
        21=>("DEC32",UnaryKind::Dec,program.dec),
        22=>("NEG32",UnaryKind::Neg,program.neg),
        _=>return None,
    };
    let links_after_build=program.links_after_build as u32;

    let bits=bit_handles(&compiler,32,value);
    let word=materialize_exact_sequence(&mut compiler.store,&bits).ok()?;
    let args=materialize_exact_sequence(&mut compiler.store,&[word]).ok()?;
    let invocation=call(&mut compiler.store,compiler.apply,function,args);
    let initial=compiler.store.ensure_pair(compiler.k,invocation).ok()?;

    let prepared_roots=vec![
        semantic_source(&compiler.store,"function.unary.selected",function),
        semantic_source(&compiler.store,"function.unary.inc",program.inc),
        semantic_source(&compiler.store,"function.unary.dec",program.dec),
        semantic_source(&compiler.store,"function.unary.neg",program.neg),
        semantic_source(
            &compiler.store,"function.flagged_arithmetic",program.flagged.flagged,
        ),
        semantic_source(
            &compiler.store,"result.flagged_tag",program.flagged.result_tag,
        ),
        semantic_source(&compiler.store,"data.value.word",word),
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
        semantic_source(&compiler.store,"result.flag.cf",program.schema.cf),
        semantic_source(&compiler.store,"result.flag.pf",program.schema.pf),
        semantic_source(&compiler.store,"result.flag.af",program.schema.af),
        semantic_source(&compiler.store,"result.flag.zf",program.schema.zf),
        semantic_source(&compiler.store,"result.flag.sf",program.schema.sf),
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
    let cf_flag=loaded_handle(&load,"result.flag.cf")?;
    let pf_flag=loaded_handle(&load,"result.flag.pf")?;
    let af_flag=loaded_handle(&load,"result.flag.af")?;
    let zf_flag=loaded_handle(&load,"result.flag.zf")?;
    let sf_flag=loaded_handle(&load,"result.flag.sf")?;
    let of_flag=loaded_handle(&load,"result.flag.of")?;

    let max_steps=program.active_steps as u32+2;
    let (mut engine,execute)=execute_to_quiescence(
        &mut memory,interpreter,initial,32,max_steps,
    )?;
    if execute.active_reaction_count!=program.active_steps as u32
        ||engine.current().len()!=1
    {
        return None;
    }

    let final_link=engine.current()[0];
    let (actual,payload)=decode_runtime_unary_effect(
        &memory,
        final_link,
        caller,
        result_tag,
        zero,
        one,
        set_tag,
        cf_flag,
        pf_flag,
        af_flag,
        zf_flag,
        sf_flag,
        of_flag,
    )?;
    let oracle=expected(32,kind,value);

    let result_anum=memory.store.export_anum(final_link).ok()?;
    let result_sequence_anum=memory.store.export_anum(payload).ok()?;
    let links_after_first=memory.store.link_count() as u32;
    let identical_rerun_link_delta=identical_rerun(
        &mut memory,&mut engine,initial,&result_anum,max_steps,
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

    let mut defined_mask=WEB_PF|WEB_AF|WEB_ZF|WEB_SF|WEB_OF;
    let mut value_mask=0u32;
    for (flag_mask,bit) in [
        (WEB_PF,actual.pf),
        (WEB_AF,actual.af),
        (WEB_ZF,actual.zf),
        (WEB_SF,actual.sf),
        (WEB_OF,actual.of),
    ]{
        if bit!=0{value_mask|=flag_mask;}
    }
    let preserve_mask=if let Some(cf)=actual.cf{
        defined_mask|=WEB_CF;
        if cf!=0{value_mask|=WEB_CF;}
        0
    }else{
        WEB_CF
    };

    let outcome=WebUnaryOutcome{
        value:actual.value,
        writeback:1,
        defined_mask,
        value_mask,
        undefined_mask:0,
        preserve_mask,
        reactions:program.active_steps as u32,
        links_after_build,
        links_after_first,
        steady_link_delta:proof.result.identical_rerun_link_delta,
        quiescent:u8::from(proof.execute.final_quiescent),
    };

    Some(WebUnaryProofExecution{outcome,proof})
}

pub(crate) fn web_run_unary32(op:u32,value:u32)->Option<WebUnaryOutcome>{
    let mut f=FullFixture::new();
    let p=Program::install(&mut f,32);
    let function=match op{20=>p.inc,21=>p.dec,22=>p.neg,_=>return None};
    let links_after_build=f.store.link_count() as u32;

    let first=run(&mut f,&p,function,value);
    let links_after_first=f.store.link_count() as u32;
    let second=run(&mut f,&p,function,value);
    assert_eq!(second,first,"web unary repeat changed result");
    let links_after_second=f.store.link_count() as u32;

    let mut defined_mask=WEB_PF|WEB_AF|WEB_ZF|WEB_SF|WEB_OF;
    let mut value_mask=0u32;
    for (mask,bit) in [(WEB_PF,first.pf),(WEB_AF,first.af),(WEB_ZF,first.zf),(WEB_SF,first.sf),(WEB_OF,first.of)]{
        if bit!=0{value_mask|=mask;}
    }
    let preserve_mask;
    if let Some(cf)=first.cf{
        defined_mask|=WEB_CF;
        if cf!=0{value_mask|=WEB_CF;}
        preserve_mask=0;
    }else{
        preserve_mask=WEB_CF;
    }

    Some(WebUnaryOutcome{
        value:first.value,
        writeback:1,
        defined_mask,
        value_mask,
        undefined_mask:0,
        preserve_mask,
        reactions:p.active_steps as u32,
        links_after_build,
        links_after_first,
        steady_link_delta:links_after_second-links_after_first,
        quiescent:1,
    })
}


#[test]
fn persistent_unary32_session_lifecycle_all_ops(){
    for (op,kind,run_values) in [
        (
            20u32,
            UnaryKind::Inc,
            [0x7fff_ffffu32,0xffff_ffff,0,0x8000_0000,0x7fff_ffff],
        ),
        (
            21u32,
            UnaryKind::Dec,
            [0x8000_0000u32,0,1,0xffff_ffff,0x8000_0000],
        ),
        (
            22u32,
            UnaryKind::Neg,
            [0u32,1,0x8000_0000,0xffff_ffff,0],
        ),
    ]{
        let prepare=prepare_unary32_session_program()
            .expect("prepare UNARY32");
        let prepared_roles=prepare.semantic_roots.iter()
            .map(|root|root.role.as_str())
            .collect::<std::collections::BTreeSet<_>>();

        for runtime_role in [
            "function.unary.selected",
            "data.value.word",
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
            "function.unary.inc",
            "function.unary.dec",
            "function.unary.neg",
            "function.flagged_arithmetic",
        ]{
            assert!(
                prepared_roles.contains(static_role),
                "op {op}: static UNARY32 Aset missing {static_role}",
            );
        }

        let (mut session,load)=load_runtime_session(&prepare,64)
            .expect("load UNARY32");
        let memory_id=session.memory.id.clone();
        let base_link_count=session.base_link_count;
        let base_carrier=session.memory.store.export_packed_duplets();

        let mut oracle_fixture=FullFixture::new();
        let oracle_program=Program::install(&mut oracle_fixture,32);
        let oracle_function=match kind{
            UnaryKind::Inc=>oracle_program.inc,
            UnaryKind::Dec=>oracle_program.dec,
            UnaryKind::Neg=>oracle_program.neg,
        };

        let mut first_projection=None;
        for (index,value) in run_values.into_iter().enumerate(){
            let (initial,before,after)=configure_unary32_session(
                &mut session,&load,op,value,
            ).expect("configure UNARY32");
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
                &mut session,
                initial,
                oracle_program.active_steps as u32 + 2,
            ).expect("execute UNARY32");
            assert!(execute.final_quiescent);
            assert_eq!(session.memory.id,memory_id);
            assert_eq!(session.engine.current().len(),1);
            assert_eq!(
                execute.active_reaction_count,
                oracle_program.active_steps as u32,
                "op {op} run {index}: reaction count",
            );

            let projected=project_unary32_session_result(
                &session,&load,
            ).expect("project UNARY32");
            let expected=expected(32,kind,value);

            let mut expected_defined=
                WEB_PF|WEB_AF|WEB_ZF|WEB_SF|WEB_OF;
            let mut expected_values=0u32;
            for (flag_mask,bit) in [
                (WEB_PF,expected.pf),
                (WEB_AF,expected.af),
                (WEB_ZF,expected.zf),
                (WEB_SF,expected.sf),
                (WEB_OF,expected.of),
            ]{
                if bit!=0{expected_values|=flag_mask;}
            }
            let expected_preserve=if let Some(cf)=expected.cf{
                expected_defined|=WEB_CF;
                if cf!=0{expected_values|=WEB_CF;}
                0
            }else{
                WEB_CF
            };

            assert_eq!(projected.value,expected.value);
            assert_eq!(projected.writeback,1);
            assert_eq!(projected.defined_mask,expected_defined);
            assert_eq!(projected.value_mask,expected_values);
            assert_eq!(projected.undefined_mask,0);
            assert_eq!(projected.preserve_mask,expected_preserve);

            let carrier=session.memory.store.export_packed_duplets();
            assert_eq!(
                &carrier[..base_link_count],base_carrier.as_slice(),
                "op {op}: loaded base prefix changed",
            );

            // The runtime result must be the same semantic endpoint as the
            // existing fresh structural execution for the same unary function.
            let fresh=run(
                &mut oracle_fixture,&oracle_program,oracle_function,value,
            );
            assert_eq!(fresh,expected);

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
#[ignore="heavy M4 unary arithmetic suite; mandatory release workflow"]
fn m4_unary_arith_8_16_32_reuses_flagged_add_sub(){
    for width in [8usize,16,32]{
        let mut f=FullFixture::new(); let p=Program::install(&mut f,width);
        assert_eq!(p.active_steps,p.flagged.active_steps+2);
        for a in vectors(width){
            for(kind,function)in[
                (UnaryKind::Inc,p.inc),(UnaryKind::Dec,p.dec),(UnaryKind::Neg,p.neg)
            ]{
                assert_eq!(run(&mut f,&p,function,a),expected(width,kind,a),
                    "width={width} kind={kind:?} a={a:#x}");
            }
        }
        println!("M4_UNARY width={} steps={} links={}",width,p.active_steps,p.links_after_build);
    }
}

#[test]
#[ignore="heavy M4 unary arithmetic suite; mandatory release workflow"]
fn m4_unary_arith_cf_preserve_neg_edges_and_steady(){
    let mut f=FullFixture::new(); let p=Program::install(&mut f,32);

    let inc=run(&mut f,&p,p.inc,0x7fff_ffff);
    assert_eq!(inc.of,1); assert_eq!(inc.cf,None); assert_eq!(inc.patch_len,5);

    let dec=run(&mut f,&p,p.dec,0x8000_0000);
    assert_eq!(dec.of,1); assert_eq!(dec.cf,None); assert_eq!(dec.patch_len,5);

    let neg0=run(&mut f,&p,p.neg,0);
    assert_eq!(neg0.cf,Some(0)); assert_eq!(neg0.value,0);

    let neg1=run(&mut f,&p,p.neg,1);
    assert_eq!(neg1.cf,Some(1)); assert_eq!(neg1.value,u32::MAX);

    let negmin=run(&mut f,&p,p.neg,0x8000_0000);
    assert_eq!(negmin.of,1); assert_eq!(negmin.cf,Some(1));

    let first=run(&mut f,&p,p.dec,0x1234_5678);
    let links=f.store.link_count();
    let second=run(&mut f,&p,p.dec,0x1234_5678);
    assert_eq!(first,second);
    assert_eq!(f.store.link_count(),links,
        "repeated identical DEC32 materialized new Links");
}
