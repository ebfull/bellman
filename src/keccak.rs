use super::bit::Bit;
use std::slice::IterMut;

const KECCAKF_RNDC: [u64; 24] = 
[
    0x0000000000000001, 0x0000000000008082, 0x800000000000808a,
    0x8000000080008000, 0x000000000000808b, 0x0000000080000001,
    0x8000000080008081, 0x8000000000008009, 0x000000000000008a,
    0x0000000000000088, 0x0000000080008009, 0x000000008000000a,
    0x000000008000808b, 0x800000000000008b, 0x8000000000008089,
    0x8000000000008003, 0x8000000000008002, 0x8000000000000080, 
    0x000000000000800a, 0x800000008000000a, 0x8000000080008081,
    0x8000000000008080, 0x0000000080000001, 0x8000000080008008
];

const KECCAKF_ROTC: [usize; 24] = 
[
    1,  3,  6,  10, 15, 21, 28, 36, 45, 55, 2,  14, 
    27, 41, 56, 8,  25, 43, 62, 18, 39, 61, 20, 44
];

const KECCAKF_PILN: [usize; 24] = 
[
    10, 7,  11, 17, 18, 3, 5,  16, 8,  21, 24, 4, 
    15, 23, 19, 13, 12, 2, 20, 14, 22, 9,  6,  1 
];

fn keccakf(st: &mut [Byte], rounds: usize)
{
    use std::borrow::Borrow;

    struct State<B: Borrow<Bit>> {
        bits: Vec<B>
    }

    impl<'a> State<&'a mut Bit> {
        fn new(bytes: &'a mut [Byte]) -> State<&'a mut Bit> {
            State {
                bits: bytes.iter_mut()
                            .rev() // Endianness
                            .flat_map(|b| b.iter_mut())
                            .collect()
            }
        }

        fn set(&mut self, to: State<Bit>) {
            for (a, b) in self.bits.iter_mut()
                                   .zip(to.bits.into_iter()) {
                **a = b;
            }
        }
    }

    impl From<u64> for State<Bit> {
        fn from(num: u64) -> State<Bit> {
            fn bit_at(num: u64, i: usize) -> bool {
                ((num << i) >> 63) == 1
            }

            State {
                bits: (0..64).map(|i| Bit::constant(bit_at(num, i))).collect()
            }
        }
    }

    impl<A: Borrow<Bit>> State<A> {
        fn duplicate(&self) -> State<Bit> {
            State {
                bits: self.bits.iter().map(|a| a.borrow())
                                      .map(|a| (*a).clone())
                                      .collect()
            }
        }

        fn binary_map<F, B>(&self, other: &State<B>, f: F) -> State<Bit>
            where F: Fn(&Bit, &Bit) -> Bit, B: Borrow<Bit>
        {
            State {
                bits: self.bits.iter().map(|a| a.borrow())
                                        .zip(other.bits.iter().map(|a| a.borrow()))
                                        .map(|(a, b)| f(a, b))
                                        .collect()
            }
        }

        fn xor<B: Borrow<Bit>>(&self, other: &State<B>) -> State<Bit> {
            self.binary_map(other, Bit::xor)
        }

        fn notand<B: Borrow<Bit>>(&self, other: &State<B>) -> State<Bit> {
            self.binary_map(other, Bit::notand)
        }

        fn rotl(&self, by: usize) -> State<Bit> {
            let by = by % 64;

            State {
                bits: self.bits[by..].iter().map(|a| a.borrow())
                                     .chain(self.bits[0..by].iter().map(|a| a.borrow()))
                                     .cloned()
                                     .collect()
            }
        }
    }

    let mut st: Vec<_> = st.chunks_mut(8).map(|c| State::new(c)).collect();

    for round in 0..rounds {
        /*
        // Theta
        for (i = 0; i < 5; i++)     
            bc[i] = st[i] ^ st[i + 5] ^ st[i + 10] ^ st[i + 15] ^ st[i + 20];
        */

        let mut bc: Vec<State<Bit>> = (0..5).map(|i| st[i]
                                                     .xor(&st[i+5])
                                                     .xor(&st[i+10])
                                                     .xor(&st[i+15])
                                                     .xor(&st[i+20])
                                                ).collect();

        /*
        for (i = 0; i < 5; i++) {
            t = bc[(i + 4) % 5] ^ ROTL64(bc[(i + 1) % 5], 1);
            for (j = 0; j < 25; j += 5)
                st[j + i] ^= t;
        }
        */

        for i in 0..5 {
            let tmp = bc[(i + 4) % 5].xor(&bc[(i + 1) % 5].rotl(1));

            for j in (0..25).filter(|a| a % 5 == 0) {
                let new = tmp.xor(&st[j + i]);
                st[j + i].set(new);
            }
        }

        {
            /*
            // Rho Pi
            t = st[1];
            for (i = 0; i < 24; i++) {
                j = keccakf_piln[i];
                bc[0] = st[j];
                st[j] = ROTL64(t, keccakf_rotc[i]);
                t = bc[0];
            }
            */
            let mut tmp = st[1].duplicate();

            for i in 0..24 {
                let j = KECCAKF_PILN[i];

                bc[0] = st[j].duplicate();
                st[j].set(tmp.rotl(KECCAKF_ROTC[i]));
                tmp = bc[0].duplicate();
            }
        }

        {
            /*
            //  Chi
            for (j = 0; j < 25; j += 5) {
                for (i = 0; i < 5; i++)
                    bc[i] = st[j + i];
                for (i = 0; i < 5; i++)
                    st[j + i] ^= (~bc[(i + 1) % 5]) & bc[(i + 2) % 5];
            }
            */

            for j in (0..25).filter(|a| a % 5 == 0) {
                for i in 0..5 {
                    bc[i] = st[j + i].duplicate();
                }

                for i in 0..5 {
                    let n = st[j + i].xor(&bc[(i + 1) % 5].notand(&bc[(i + 2) % 5]));
                    st[j + i].set(n);
                }
            }
        }

        /*
        //  Iota
        st[0] ^= keccakf_rndc[round];
        */

        let n = st[0].xor(&KECCAKF_RNDC[round].into());
        st[0].set(n);
    }
}

pub fn sha3_256(message: &[Byte]) -> Vec<Bit> {
    // As defined by FIPS202
    keccak(1088, 512, message, 0x06, 32, 24)
}

fn keccak(rate: usize, capacity: usize, mut input: &[Byte], delimited_suffix: u8, mut mdlen: usize, num_rounds: usize)
    -> Vec<Bit>
{
    use std::cmp::min;

    let mut st: Vec<Byte> = Some(Byte::new(0)).into_iter().cycle().take(200).collect();

    let rate_in_bytes = rate / 8;
    let mut input_byte_len = input.len();
    let mut block_size = 0;

    if ((rate + capacity) != 1600) || ((rate % 8) != 0) {
        panic!("invalid parameters");
    }

    while input_byte_len > 0 {
        block_size = min(input_byte_len, rate_in_bytes);

        for i in 0..block_size {
            st[i] = st[i].xor(&input[i]);
        }

        input = &input[block_size..];
        input_byte_len -= block_size;

        if block_size == rate_in_bytes {
            keccakf(&mut st, num_rounds);
            block_size = 0;
        }
    }

    st[block_size] = st[block_size].xor(&Byte::new(delimited_suffix));

    if ((delimited_suffix & 0x80) != 0) && (block_size == (rate_in_bytes-1)) {
        keccakf(&mut st, num_rounds);
    }

    st[rate_in_bytes-1] = st[rate_in_bytes-1].xor(&Byte::new(0x80));

    keccakf(&mut st, num_rounds);

    let mut output = Vec::with_capacity(mdlen);

    while mdlen > 0 {
        block_size = min(mdlen, rate_in_bytes);
        output.extend_from_slice(&st[0..block_size]);
        mdlen -= block_size;

        if mdlen > 0 {
            keccakf(&mut st, num_rounds);
        }
    }

    output.into_iter().flat_map(|byte| byte.bits.into_iter()).collect()
}

#[test]
fn test_sha3_256() {
    use super::circuit::{CircuitBuilder,Equals};
    use super::variable::Var;
    use tinysnark::{self,FieldT};

    let test_vector: Vec<(Vec<u8>, [u8; 32])> = vec![
        (vec![0xff],
         [0x44,0x4b,0x89,0xec,0xce,0x39,0x5a,0xec,0x5d,0xc9,0x8f,0x19,0xde,0xfd,0x3a,0x23,0xbc,0xa0,0x82,0x2f,0xc7,0x22,0x26,0xf5,0x8c,0xa4,0x6a,0x17,0xee,0xec,0xa4,0x42]
        ),
        (vec![0x00],
         [0x5d,0x53,0x46,0x9f,0x20,0xfe,0xf4,0xf8,0xea,0xb5,0x2b,0x88,0x04,0x4e,0xde,0x69,0xc7,0x7a,0x6a,0x68,0xa6,0x07,0x28,0x60,0x9f,0xc4,0xa6,0x5f,0xf5,0x31,0xe7,0xd0]
        ),
        (vec![0x30, 0x31, 0x30, 0x31],
         [0xe5,0xbf,0x4a,0xd7,0xda,0x2b,0x4d,0x64,0x0d,0x2b,0x8d,0xd3,0xae,0x9b,0x6e,0x71,0xb3,0x6e,0x0f,0x3d,0xb7,0x6a,0x1e,0xc0,0xad,0x6b,0x87,0x2f,0x3e,0xcc,0x2e,0xbc]
        ),
        (vec![0x30],
         [0xf9,0xe2,0xea,0xaa,0x42,0xd9,0xfe,0x9e,0x55,0x8a,0x9b,0x8e,0xf1,0xbf,0x36,0x6f,0x19,0x0a,0xac,0xaa,0x83,0xba,0xd2,0x64,0x1e,0xe1,0x06,0xe9,0x04,0x10,0x96,0xe4]
        ),
        (vec![0x30,0x30],
         [0x2e,0x16,0xaa,0xb4,0x83,0xcb,0x95,0x57,0x7c,0x50,0xd3,0x8c,0x8d,0x0d,0x70,0x40,0xf4,0x67,0x26,0x83,0x23,0x84,0x46,0xc9,0x90,0xba,0xbb,0xca,0x5a,0xe1,0x33,0xc8]
        ),
        ((0..64).map(|_| 0x30).collect::<Vec<_>>(),
         [0xc6,0xfd,0xd7,0xa7,0xf7,0x08,0x62,0xb3,0x6a,0x26,0xcc,0xd1,0x47,0x52,0x26,0x80,0x61,0xe9,0x81,0x03,0x29,0x9b,0x28,0xfe,0x77,0x63,0xbd,0x96,0x29,0x92,0x6f,0x4b]
        ),
        ((0..128).map(|_| 0x30).collect::<Vec<_>>(),
         [0x99,0x9d,0xb4,0xd4,0x28,0x7b,0x52,0x15,0x20,0x8d,0x11,0xe4,0x0a,0x27,0xca,0x54,0xac,0xa0,0x09,0xb2,0x5c,0x4f,0x7a,0xb9,0x1a,0xd8,0xaa,0x93,0x60,0xf0,0x63,0x71]
        ),
        ((0..256).map(|_| 0x30).collect::<Vec<_>>(),
         [0x11,0xea,0x74,0x37,0x7b,0x74,0xf1,0x53,0x9f,0x2e,0xd9,0x0a,0xb8,0xca,0x9e,0xb1,0xe0,0x70,0x8a,0x4b,0xfb,0xad,0x4e,0x81,0xcc,0x77,0xd9,0xa1,0x61,0x9a,0x10,0xdb]
        ),
        ((0..512).map(|_| 0x30).collect::<Vec<_>>(),
         [0x1c,0x80,0x1b,0x16,0x3a,0x2a,0xbe,0xd0,0xe8,0x07,0x1e,0x7f,0xf2,0x60,0x4e,0x98,0x11,0x22,0x80,0x54,0x14,0xf3,0xc8,0xfd,0x96,0x59,0x5d,0x7e,0xe1,0xd6,0x54,0xe2]
        ),
        ((0..64).map(|_| 0x00).collect::<Vec<_>>(),
         [0x07,0x0f,0xa1,0xab,0x6f,0xcc,0x55,0x7e,0xd1,0x4d,0x42,0x94,0x1f,0x19,0x67,0x69,0x30,0x48,0x55,0x1e,0xb9,0x04,0x2a,0x8d,0x0a,0x05,0x7a,0xfb,0xd7,0x5e,0x81,0xe0]
        ),
    ];

    for (i, &(ref message, ref expected)) in test_vector.iter().enumerate() {
        let message: Vec<Byte> = message.iter().map(|a| Byte::new(*a)).collect();
        let result: Vec<u8> = sha3_256(&message)
                                 .chunks(8)
                                 .map(|a| Byte::from(a))
                                 .map(|a| a.unwrap_constant())
                                 .collect();

        if &*result != expected {
            print!("Got: ");
            for i in result.iter() {
                print!("0x{:02x},", i);
            }
            print!("\nExpected: ");
            for i in expected.iter() {
                print!("0x{:02x},", i);
            }
            println!("");
            panic!("Hash {} failed!", i+1);
        } else {
            //println!("--- HASH {} SUCCESS ---", i+1);
        }
    }

    if option_env!("DEBUG_NOPROVE").is_some() {
        return;
    }

    tinysnark::init();

    for (i, &(ref message, ref expected)) in test_vector.iter().enumerate() {
        fn into_bytes(a: &[Var]) -> Vec<Byte> {
            let a: Vec<_> = a.into_iter().map(|a| Bit::new(a)).collect();
            
            a.chunks(8).map(|a| Byte::from(a)).collect()
        }

        fn into_fieldt(a: &[u8], vars: &mut [FieldT]) {
            let mut counter = 0;

            for byte in a {
                for bit in (0..8).map(|i| byte & (1 << i) != 0).rev() {
                    if bit { vars[counter] = FieldT::one() } else { vars[counter] = FieldT::zero() }
                    counter += 1;
                }
            }
        }

        let (public, private, mut circuit) = CircuitBuilder::new(expected.len() * 8, message.len() * 8);

        let private = into_bytes(&private);

        circuit.constrain(sha3_256(&private).must_equal(&public));

        let circuit = circuit.finalize();

        let mut input: Vec<FieldT> = (0..message.len() * 8).map(|_| FieldT::zero()).collect();
        let mut output: Vec<FieldT> = (0..expected.len() * 8).map(|_| FieldT::zero()).collect();

        into_fieldt(message, &mut input);
        into_fieldt(expected, &mut output);

        let proof = circuit.prove(&output, &input).unwrap();
        assert!(circuit.verify(&proof, &output));
    }
}

#[derive(Clone)]
pub struct Byte {
    bits: Vec<Bit>
}

impl<'a> From<&'a [Bit]> for Byte {
    fn from(a: &'a [Bit]) -> Byte {
        assert_eq!(8, a.len());

        Byte {
            bits: a.to_owned()
        }
    }
}

impl Byte {
    pub fn bits(&self) -> Vec<Bit> {
        self.bits.clone()
    }

    pub fn new(byte: u8) -> Byte {
        Byte {
            bits: (0..8).map(|i| Bit::constant(byte & (1 << i) != 0))
                        .rev()
                        .collect()
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<Bit> {
        self.bits.iter_mut()
    }

    pub fn unwrap_constant(&self) -> u8 {
        let mut cur = 7;
        let mut acc = 0;

        for bit in &self.bits {
            match bit {
                &Bit::Constant(true) => {
                    acc |= 1 << cur;
                },
                &Bit::Constant(false) => {},
                _ => panic!("Tried to unwrap a constant from a non-constant")
            }
            cur -= 1;
        }

        acc
    }

    pub fn xor(&self, other: &Byte) -> Byte {
        Byte {
            bits: self.bits.iter()
                           .zip(other.bits.iter())
                           .map(|(a, b)| a.xor(b))
                           .collect()
        }
    }
}