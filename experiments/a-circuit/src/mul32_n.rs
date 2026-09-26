use super::{
    full_adder::{
        call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
    },
    wide64_n::Wide64Program,
};
use amemory_optimized_cpu_probe::{
    structural::{materialize_exact_sequence, read_exact_sequence},
    Handle, OptimizedLinkStore,
};

const WIDTH: usize = 32;

struct AnchorGen{
    current:Handle,
    flip:bool,
    o:Handle,
    c:Handle,
}
impl AnchorGen{
    fn new(
        store:&mut OptimizedLinkStore,
        mut seed:Handle,
        o:Handle,
        c:Handle,
    )->Self{
        for pole in [o,o,c,c,o,c,o,c,o,o,c,o,c,c,o,o,c]{
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
    fn roles(&mut self,store:&mut OptimizedLinkStore,count:usize)->Vec<Handle>{
        (0..count).map(|_|self.next(store)).collect()
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

fn partial_wide(
    f:&mut FullFixture,
    a_bits:&[Handle],
    shift:usize,
)->Handle{
    assert_eq!(a_bits.len(),WIDTH);
    assert!(shift<WIDTH);

    let mut bits64=Vec::with_capacity(64);
    for pos in 0..64usize{
        if pos>=shift && pos<shift+WIDTH{
            bits64.push(a_bits[pos-shift]);
        }else{
            bits64.push(f.zero);
        }
    }
    let lo=materialize_exact_sequence(&mut f.store,&bits64[..32]).unwrap();
    let hi=materialize_exact_sequence(&mut f.store,&bits64[32..]).unwrap();
    materialize_exact_sequence(&mut f.store,&[lo,hi]).unwrap()
}

fn zero_wide(f:&mut FullFixture)->Handle{
    let zeros=vec![f.zero;WIDTH];
    let word=materialize_exact_sequence(&mut f.store,&zeros).unwrap();
    materialize_exact_sequence(&mut f.store,&[word,word]).unwrap()
}

#[derive(Clone,Debug)]
pub(crate) struct Mul32Program{
    pub(crate) mul32:Handle,
    pub(crate) result_tag:Handle,
    add64:Wide64Program,
    pub(crate) links_after_build:usize,
}

impl Mul32Program{
    pub(crate) fn install(f:&mut FullFixture)->Self{
        let add64=Wide64Program::install(f);
        let seed=f.store.ensure_pair(add64.add64,add64.result_tag).unwrap();
        let mut a=AnchorGen::new(&mut f.store,seed,f.o,f.c);

        let l=a.next(&mut f.store); let r=a.next(&mut f.store);
        let mul32=f.store.ensure_pair(l,r).unwrap();
        let l=a.next(&mut f.store); let r=a.next(&mut f.store);
        let result_tag=f.store.ensure_pair(l,r).unwrap();

        let mut stages=Vec::with_capacity(WIDTH);
        let mut add_tags=Vec::with_capacity(WIDTH);
        for _ in 0..WIDTH{
            let l=a.next(&mut f.store); let r=a.next(&mut f.store);
            stages.push(f.store.ensure_pair(l,r).unwrap());
            let l=a.next(&mut f.store); let r=a.next(&mut f.store);
            add_tags.push(f.store.ensure_pair(l,r).unwrap());
        }

        let zero64=zero_wide(f);

        // MUL OPEN -> stage 0 with zero accumulator.
        {
            let k=a.next(&mut f.store);
            let a_bits=a.roles(&mut f.store,WIDTH);
            let b_bits=a.roles(&mut f.store,WIDTH);
            let aword=materialize_exact_sequence(&mut f.store,&a_bits).unwrap();
            let bword=materialize_exact_sequence(&mut f.store,&b_bits).unwrap();

            let args=materialize_exact_sequence(&mut f.store,&[aword,bword]).unwrap();
            let before_call=call(&mut f.store,f.apply,mul32,args);
            let before=f.store.ensure_pair(k,before_call).unwrap();

            let stage_args=materialize_exact_sequence(
                &mut f.store,&[aword,bword,zero64]).unwrap();
            let stage_call=call(&mut f.store,f.apply,stages[0],stage_args);
            let after=f.store.ensure_pair(k,stage_call).unwrap();

            let mut roles=Vec::with_capacity(1+2*WIDTH);
            roles.push(k);
            roles.extend_from_slice(&a_bits);
            roles.extend_from_slice(&b_bits);

            let(_,admission)=define_bundle_rule(
                &mut f.store,f.theory,&roles,before,&[after]);
            index_rule_for(&mut f.store,&[f.o],admission);
        }

        for i in 0..WIDTH{
            for bit_value in 0u8..=1{
                let k=a.next(&mut f.store);
                let a_bits=a.roles(&mut f.store,WIDTH);

                let mut b_bits=Vec::with_capacity(WIDTH);
                let mut b_roles=Vec::with_capacity(WIDTH-1);
                for pos in 0..WIDTH{
                    if pos==i{
                        b_bits.push(if bit_value==0{f.zero}else{f.one});
                    }else{
                        let role=a.next(&mut f.store);
                        b_bits.push(role);
                        b_roles.push(role);
                    }
                }
                let acc=a.next(&mut f.store);

                let aword=materialize_exact_sequence(&mut f.store,&a_bits).unwrap();
                let bword=materialize_exact_sequence(&mut f.store,&b_bits).unwrap();
                let stage_args=materialize_exact_sequence(
                    &mut f.store,&[aword,bword,acc]).unwrap();
                let stage_call=call(&mut f.store,f.apply,stages[i],stage_args);
                let before=f.store.ensure_pair(k,stage_call).unwrap();

                let after=if bit_value==0{
                    if i+1<WIDTH{
                        let next_args=materialize_exact_sequence(
                            &mut f.store,&[aword,bword,acc]).unwrap();
                        let next_call=call(
                            &mut f.store,f.apply,stages[i+1],next_args);
                        f.store.ensure_pair(k,next_call).unwrap()
                    }else{
                        let payload=materialize_exact_sequence(
                            &mut f.store,&[acc]).unwrap();
                        let endpoint=f.store.ensure_pair(result_tag,payload).unwrap();
                        f.store.ensure_pair(k,endpoint).unwrap()
                    }
                }else{
                    let partial=partial_wide(f,&a_bits,i);
                    let add_args=materialize_exact_sequence(
                        &mut f.store,&[acc,partial,f.zero]).unwrap();
                    let add_call=call(
                        &mut f.store,f.apply,add64.add64,add_args);
                    let caller=stage_frame(
                        &mut f.store,add_tags[i],&[k,aword,bword]);
                    f.store.ensure_pair(caller,add_call).unwrap()
                };

                let mut roles=Vec::with_capacity(2*WIDTH+1);
                roles.push(k);
                roles.extend_from_slice(&a_bits);
                roles.extend_from_slice(&b_roles);
                roles.push(acc);

                let(_,admission)=define_bundle_rule(
                    &mut f.store,f.theory,&roles,before,&[after]);
                index_rule_for(&mut f.store,&[f.o],admission);
            }

            // ADD64 return for the set-bit branch. Cout is constrained to zero.
            {
                let k=a.next(&mut f.store);
                let a_bits=a.roles(&mut f.store,WIDTH);
                let b_bits=a.roles(&mut f.store,WIDTH);
                let sum64=a.next(&mut f.store);

                let aword=materialize_exact_sequence(&mut f.store,&a_bits).unwrap();
                let bword=materialize_exact_sequence(&mut f.store,&b_bits).unwrap();
                let caller=stage_frame(
                    &mut f.store,add_tags[i],&[k,aword,bword]);

                let add_payload=materialize_exact_sequence(
                    &mut f.store,&[sum64,f.zero]).unwrap();
                let add_endpoint=f.store.ensure_pair(
                    add64.result_tag,add_payload).unwrap();
                let before=f.store.ensure_pair(caller,add_endpoint).unwrap();

                let after=if i+1<WIDTH{
                    let next_args=materialize_exact_sequence(
                        &mut f.store,&[aword,bword,sum64]).unwrap();
                    let next_call=call(
                        &mut f.store,f.apply,stages[i+1],next_args);
                    f.store.ensure_pair(k,next_call).unwrap()
                }else{
                    let payload=materialize_exact_sequence(
                        &mut f.store,&[sum64]).unwrap();
                    let endpoint=f.store.ensure_pair(result_tag,payload).unwrap();
                    f.store.ensure_pair(k,endpoint).unwrap()
                };

                let mut roles=Vec::with_capacity(2+2*WIDTH);
                roles.push(k);
                roles.extend_from_slice(&a_bits);
                roles.extend_from_slice(&b_bits);
                roles.push(sum64);

                let(_,admission)=define_bundle_rule(
                    &mut f.store,f.theory,&roles,before,&[after]);
                index_rule_for(
                    &mut f.store,&[add64.result_tag],admission);
            }
        }

        Self{
            mul32,result_tag,add64,
            links_after_build:f.store.link_count(),
        }
    }
}

fn word_handle(f:&mut FullFixture,value:u32)->Handle{
    let bits:(Vec<Handle>)=(0..WIDTH)
        .map(|i|if(value>>i)&1==1{f.one}else{f.zero})
        .collect();
    materialize_exact_sequence(&mut f.store,&bits).unwrap()
}

fn decode_bit(f:&FullFixture,h:Handle)->u8{
    if h==f.one{1}else{assert_eq!(h,f.zero);0}
}

fn decode_word(f:&FullFixture,h:Handle)->u32{
    let bits=read_exact_sequence(&f.store,h).unwrap();
    assert_eq!(bits.len(),WIDTH);
    let mut v=0u32;
    for(i,b)in bits.into_iter().enumerate(){
        v|=u32::from(decode_bit(f,b))<<i;
    }
    v
}

fn decode_wide(f:&FullFixture,h:Handle)->u64{
    let halves=read_exact_sequence(&f.store,h).unwrap();
    assert_eq!(halves.len(),2);
    u64::from(decode_word(f,halves[0]))
        |(u64::from(decode_word(f,halves[1]))<<32)
}

fn expected_steps(p:&Mul32Program,b:u32)->usize{
    33+(p.add64.active_steps+1)*(b.count_ones()as usize)
}

fn run(
    f:&mut FullFixture,
    p:&Mul32Program,
    av:u32,
    bv:u32,
)->(u64,usize){
    let aword=word_handle(f,av);
    let bword=word_handle(f,bv);
    let args=materialize_exact_sequence(&mut f.store,&[aword,bword]).unwrap();
    let invocation=call(&mut f.store,f.apply,p.mul32,args);
    let initial=f.store.ensure_pair(f.k,invocation).unwrap();
    f.engine.set_current(&f.store,&[initial]).unwrap();

    let steps=expected_steps(p,bv);
    for step in 0..steps{
        let r=f.engine.run(&mut f.store).unwrap();
        assert!(!r.quiescent,
            "MUL32 A={av:#x} B={bv:#x}: quiescent at {step}/{steps}");
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
    assert_eq!(f.engine.current().len(),1);

    let(caller,endpoint)=f.store.poles(f.engine.current()[0]).unwrap();
    assert_eq!(caller,f.k);
    let(tag,payload)=f.store.poles(endpoint).unwrap();
    assert_eq!(tag,p.result_tag);
    let values=read_exact_sequence(&f.store,payload).unwrap();
    assert_eq!(values.len(),1);

    (decode_wide(f,values[0]),steps)
}

#[test]
#[ignore="heavy structural MUL32 suite; mandatory dedicated workflow"]
fn m4_mul32_shift_add_edges_and_reaction_formula(){
    let mut f=FullFixture::new();
    let p=Mul32Program::install(&mut f);
    assert_eq!(p.add64.active_steps,1037);

    let cases=[
        (0u32,0u32),
        (0xdead_beefu32,0u32),
        (0u32,1u32),
        (0x1234_5678u32,1u32),
        (1u32,0x8000_0000u32),
        (0x1234_5678u32,0x0001_0001u32),
        (0xdead_beefu32,0x0101_0101u32),
        (u32::MAX,u32::MAX),
        (0xaaaa_aaaau32,0x5555_5555u32),
    ];

    for (a,b) in cases{
        let(actual,steps)=run(&mut f,&p,a,b);
        assert_eq!(actual,u64::from(a)*u64::from(b),
            "A={a:#010x} B={b:#010x}");
        assert_eq!(steps,33+1038*(b.count_ones()as usize));
    }

    println!(
        "M4_MUL32 cases={} worst_steps={} program_links={}",
        cases.len(),33+1038*32,p.links_after_build
    );
}

#[test]
#[ignore="heavy structural MUL32 suite; mandatory dedicated workflow"]
fn m4_mul32_commutative_and_steady_state(){
    let mut f=FullFixture::new();
    let p=Mul32Program::install(&mut f);

    let a=0x8000_0000u32;
    let b=0x0001_0000u32;
    let(ab,_)=run(&mut f,&p,a,b);
    let(ba,_)=run(&mut f,&p,b,a);
    assert_eq!(ab,ba);
    assert_eq!(ab,u64::from(a)*u64::from(b));

    let x=0x1357_9bdfu32;
    let(first,_)=run(&mut f,&p,x,1);
    let links=f.store.link_count();
    let(second,_)=run(&mut f,&p,x,1);
    assert_eq!(second,first);
    assert_eq!(f.store.link_count(),links,
        "repeated identical MUL32 materialized new Links");
}
