use super::{
    arithmetic_n::ArithmeticProgram,
    full_adder::{
        call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
    },
};
use amemory_optimized_cpu_probe::{
    structural::{materialize_exact_sequence, read_exact_sequence},
    Handle, OptimizedLinkStore,
};

const WIDTH: usize = 32;

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
    ) -> Self {
        for pole in [c,o,c,o,o,c,c,o,o,c,o,c,c,o,c] {
            seed=store.ensure_pair(seed,pole).unwrap();
        }
        Self{current:seed,flip:false,o,c}
    }

    fn next(&mut self,store:&mut OptimizedLinkStore)->Handle{
        let pole=if self.flip{self.c}else{self.o};
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
pub(crate) struct Wide64Program{
    pub(crate) add64:Handle,
    pub(crate) result_tag:Handle,
    arithmetic:ArithmeticProgram,
    pub(crate) active_steps:usize,
    pub(crate) links_after_build:usize,
}

impl Wide64Program{
    pub(crate) fn install(f:&mut FullFixture)->Self{
        let arithmetic=ArithmeticProgram::install(f,WIDTH);
        let seed=f.store.ensure_pair(arithmetic.arithmetic,arithmetic.result_tag).unwrap();
        let mut a=AnchorGen::new(&mut f.store,seed,f.o,f.c);

        let l=a.next(&mut f.store); let r=a.next(&mut f.store);
        let add64=f.store.ensure_pair(l,r).unwrap();

        let l=a.next(&mut f.store); let r=a.next(&mut f.store);
        let result_tag=f.store.ensure_pair(l,r).unwrap();

        let low_tag=a.next(&mut f.store);
        let high_tag=a.next(&mut f.store);

        // OPEN:
        // K -> ADD64([A64,B64,Cin])
        // =>
        // LowFrame([K,Ahi,Bhi]) -> ARITH32(Alo,Blo,Cin,ADD)
        {
            let k=a.next(&mut f.store);
            let alo=a.next(&mut f.store);
            let ahi=a.next(&mut f.store);
            let blo=a.next(&mut f.store);
            let bhi=a.next(&mut f.store);
            let cin=a.next(&mut f.store);

            let awide=materialize_exact_sequence(&mut f.store,&[alo,ahi]).unwrap();
            let bwide=materialize_exact_sequence(&mut f.store,&[blo,bhi]).unwrap();
            let args=materialize_exact_sequence(&mut f.store,&[awide,bwide,cin]).unwrap();
            let before_call=call(&mut f.store,f.apply,add64,args);
            let before=f.store.ensure_pair(k,before_call).unwrap();

            let caller=stage_frame(&mut f.store,low_tag,&[k,ahi,bhi]);
            let low_args=materialize_exact_sequence(
                &mut f.store,&[alo,blo,cin,f.zero]).unwrap();
            let low_call=call(&mut f.store,f.apply,arithmetic.arithmetic,low_args);
            let after=f.store.ensure_pair(caller,low_call).unwrap();

            let (_,admission)=define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k,alo,ahi,blo,bhi,cin],
                before,
                &[after],
            );
            index_rule_for(&mut f.store,&[f.o],admission);
        }

        // Low ARITH32 result:
        // carry-out becomes the high-word ADC input structurally.
        {
            let k=a.next(&mut f.store);
            let ahi=a.next(&mut f.store);
            let bhi=a.next(&mut f.store);
            let lo_sum=a.next(&mut f.store);
            let carry=a.next(&mut f.store);
            let aux=a.next(&mut f.store);
            let sign_in=a.next(&mut f.store);
            let final_raw=a.next(&mut f.store);

            let raw=materialize_exact_sequence(
                &mut f.store,
                &[lo_sum,carry,aux,sign_in,final_raw,f.zero],
            ).unwrap();
            let endpoint=f.store.ensure_pair(arithmetic.result_tag,raw).unwrap();
            let caller=stage_frame(&mut f.store,low_tag,&[k,ahi,bhi]);
            let before=f.store.ensure_pair(caller,endpoint).unwrap();

            let next_caller=stage_frame(&mut f.store,high_tag,&[k,lo_sum]);
            let high_args=materialize_exact_sequence(
                &mut f.store,&[ahi,bhi,carry,f.zero]).unwrap();
            let high_call=call(
                &mut f.store,f.apply,arithmetic.arithmetic,high_args);
            let after=f.store.ensure_pair(next_caller,high_call).unwrap();

            let roles=[k,ahi,bhi,lo_sum,carry,aux,sign_in,final_raw];
            let (_,admission)=define_bundle_rule(
                &mut f.store,f.theory,&roles,before,&[after]);
            index_rule_for(
                &mut f.store,&[arithmetic.result_tag],admission);
        }

        // High ARITH32 result -> Wide64 + final carry.
        {
            let k=a.next(&mut f.store);
            let lo_sum=a.next(&mut f.store);
            let hi_sum=a.next(&mut f.store);
            let cout=a.next(&mut f.store);
            let aux=a.next(&mut f.store);
            let sign_in=a.next(&mut f.store);
            let final_raw=a.next(&mut f.store);

            let raw=materialize_exact_sequence(
                &mut f.store,
                &[hi_sum,cout,aux,sign_in,final_raw,f.zero],
            ).unwrap();
            let endpoint=f.store.ensure_pair(arithmetic.result_tag,raw).unwrap();
            let caller=stage_frame(&mut f.store,high_tag,&[k,lo_sum]);
            let before=f.store.ensure_pair(caller,endpoint).unwrap();

            let wide=materialize_exact_sequence(
                &mut f.store,&[lo_sum,hi_sum]).unwrap();
            let payload=materialize_exact_sequence(
                &mut f.store,&[wide,cout]).unwrap();
            let result_endpoint=f.store.ensure_pair(result_tag,payload).unwrap();
            let after=f.store.ensure_pair(k,result_endpoint).unwrap();

            let roles=[k,lo_sum,hi_sum,cout,aux,sign_in,final_raw];
            let (_,admission)=define_bundle_rule(
                &mut f.store,f.theory,&roles,before,&[after]);
            index_rule_for(
                &mut f.store,&[arithmetic.result_tag],admission);
        }

        let active_steps=1+arithmetic.active_steps+1+arithmetic.active_steps+1;

        Self{
            add64,result_tag,arithmetic,active_steps,
            links_after_build:f.store.link_count(),
        }
    }
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
struct Outcome{
    value:u64,
    cout:u8,
}

fn bit_handles(f:&FullFixture,value:u32)->Vec<Handle>{
    (0..WIDTH).map(|i|if (value>>i)&1==1{f.one}else{f.zero}).collect()
}

fn word_handle(f:&mut FullFixture,value:u32)->Handle{
    let bits=bit_handles(f,value);
    materialize_exact_sequence(&mut f.store,&bits).unwrap()
}

fn wide_handle(f:&mut FullFixture,value:u64)->Handle{
    let lo=word_handle(f,value as u32);
    let hi=word_handle(f,(value>>32)as u32);
    materialize_exact_sequence(&mut f.store,&[lo,hi]).unwrap()
}

fn decode_bit(f:&FullFixture,h:Handle)->u8{
    if h==f.one{1}else{assert_eq!(h,f.zero);0}
}

fn decode_word(f:&FullFixture,h:Handle)->u32{
    let bits=read_exact_sequence(&f.store,h).unwrap();
    assert_eq!(bits.len(),WIDTH);
    let mut value=0u32;
    for(i,b)in bits.into_iter().enumerate(){
        value|=u32::from(decode_bit(f,b))<<i;
    }
    value
}

fn decode(f:&FullFixture,p:&Wide64Program)->Outcome{
    assert_eq!(f.engine.current().len(),1);
    let(caller,endpoint)=f.store.poles(f.engine.current()[0]).unwrap();
    assert_eq!(caller,f.k);
    let(tag,payload)=f.store.poles(endpoint).unwrap();
    assert_eq!(tag,p.result_tag);
    let values=read_exact_sequence(&f.store,payload).unwrap();
    assert_eq!(values.len(),2);

    let halves=read_exact_sequence(&f.store,values[0]).unwrap();
    assert_eq!(halves.len(),2);
    let lo=decode_word(f,halves[0]);
    let hi=decode_word(f,halves[1]);

    Outcome{
        value:u64::from(lo)|(u64::from(hi)<<32),
        cout:decode_bit(f,values[1]),
    }
}

fn run(
    f:&mut FullFixture,
    p:&Wide64Program,
    a:u64,
    b:u64,
    cin:u8,
)->Outcome{
    assert!(cin<=1);
    let awide=wide_handle(f,a);
    let bwide=wide_handle(f,b);
    let cin_h=if cin==0{f.zero}else{f.one};
    let args=materialize_exact_sequence(
        &mut f.store,&[awide,bwide,cin_h]).unwrap();
    let invocation=call(&mut f.store,f.apply,p.add64,args);
    let initial=f.store.ensure_pair(f.k,invocation).unwrap();
    f.engine.set_current(&f.store,&[initial]).unwrap();

    for step in 0..p.active_steps{
        let r=f.engine.run(&mut f.store).unwrap();
        assert!(!r.quiescent,"ADD64 unexpected quiescence at {step}");
        assert_eq!(r.raw_rule_matches,1,"step {step}");
        assert_eq!(r.transitioned_members,1,"step {step}");
        assert_eq!(r.handoff_count,1,"step {step}");
        assert_eq!(r.next_members.len(),1,"step {step}");
    }

    let stable=f.engine.current_bank();
    let q=f.engine.run(&mut f.store).unwrap();
    assert!(q.quiescent);
    assert_eq!(q.raw_rule_matches,0);
    assert_eq!(q.handoff_count,0);
    assert_eq!(f.engine.current_bank(),stable);

    decode(f,p)
}

fn expected(a:u64,b:u64,cin:u8)->Outcome{
    let total=u128::from(a)+u128::from(b)+u128::from(cin);
    Outcome{
        value:total as u64,
        cout:u8::from(total>(u64::MAX as u128)),
    }
}

fn vectors()->Vec<(u64,u64,u8)>{
    let mut out=vec![
        (0,0,0),
        (0,0,1),
        (0xffff_ffff,1,0),
        (0xffff_ffff,1,1),
        (0xffff_ffff_ffff_ffff,1,0),
        (0xffff_ffff_ffff_ffff,0,1),
        (0x0000_0001_ffff_ffff,0x0000_0000_0000_0001,0),
        (0xaaaa_aaaa_5555_5555,0x5555_5555_aaaa_aaaa,0),
        (0x8000_0000_0000_0000,0x8000_0000_0000_0000,0),
    ];

    let mut z=0x9e37_79b9_7f4a_7c15u64;
    for i in 0..4u8{
        z=z.wrapping_mul(6364136223846793005).wrapping_add(1);
        let a=z;
        z=z.wrapping_mul(6364136223846793005).wrapping_add(1);
        out.push((a,z,i&1));
    }
    out
}

#[test]
#[ignore="heavy Wide64 ADD64 suite; mandatory release workflow"]
fn m4_wide64_add64_structurally_propagates_low_carry(){
    let mut f=FullFixture::new();
    let p=Wide64Program::install(&mut f);
    assert_eq!(p.arithmetic.width,WIDTH);
    assert_eq!(p.active_steps,2*p.arithmetic.active_steps+3);
    assert_eq!(p.active_steps,1037);

    let cases=vectors();
    for &(a,b,cin) in &cases{
        let actual=run(&mut f,&p,a,b,cin);
        assert_eq!(actual,expected(a,b,cin),
            "A={a:#018x} B={b:#018x} Cin={cin}");
    }

    println!(
        "M4_WIDE64_ADD64 cases={} reactions={} program_links={}",
        cases.len(),p.active_steps,p.links_after_build
    );
}

#[test]
#[ignore="heavy Wide64 ADD64 suite; mandatory release workflow"]
fn m4_wide64_add64_carry_edges_and_steady_state(){
    let mut f=FullFixture::new();
    let p=Wide64Program::install(&mut f);

    let low_carry=run(&mut f,&p,0x0000_0000_ffff_ffff,1,0);
    assert_eq!(low_carry.value,0x0000_0001_0000_0000);
    assert_eq!(low_carry.cout,0);

    let wrap=run(&mut f,&p,u64::MAX,1,0);
    assert_eq!(wrap.value,0);
    assert_eq!(wrap.cout,1);

    let first=run(&mut f,&p,0x1234_5678_9abc_def0,0x0fed_cba9_8765_4321,1);
    let links=f.store.link_count();
    let second=run(&mut f,&p,0x1234_5678_9abc_def0,0x0fed_cba9_8765_4321,1);
    assert_eq!(second,first);
    assert_eq!(f.store.link_count(),links,
        "repeated identical ADD64 materialized new Links");
}
