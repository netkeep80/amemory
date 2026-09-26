use super::{
    flag_patch::{
        install_alu_effect_result_tag, set_flag_action, FlagPatchSchema,
    },
    flags_n::FlaggedArithmeticProgram,
    full_adder::{
        call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
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
