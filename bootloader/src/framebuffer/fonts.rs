use alloc::vec::Vec;
use core::str::{Lines, SplitWhitespace};
use log::info;

static TERMINUS_U18N: &'static str = include_str!("./ter-u18n.bdf");

pub const FONT_WIDTH: usize = 16;
pub const FONT_HEIGHT: usize = 18;

pub type Bitmap = [[u8; FONT_WIDTH]; FONT_HEIGHT];

pub struct Terminus16x18Glyph {
    pub codepoint: u8,
    pub bitmap: Bitmap,
}

pub struct Terminus16x18Font {
    pub glyphs: Vec<Terminus16x18Glyph>,
}

impl Terminus16x18Font {
    pub fn get_glyph(&self, codepoint: u8) -> &Terminus16x18Glyph {
        &self.glyphs[codepoint as usize - 32]
    }
}

pub fn parse_bdf() -> Terminus16x18Font {
    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    enum Token {
        STARTCHAR,
        ENCODING,
        BITMAP,
        UNUSED,
        EMPTY,
    }
    impl core::fmt::Display for Token {
        fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
            match *self {
                Token::STARTCHAR => f.write_str("STARTCHAR"),
                Token::ENCODING => f.write_str("ENCODING"),
                Token::BITMAP => f.write_str("BITMAP"),
                Token::UNUSED => f.write_str("UNUSED"),
                Token::EMPTY => f.write_str("EMPTY"),
            }
        }
    }

    let stack = &mut Vec::<Token>::with_capacity(2);
    let mut font = Terminus16x18Font {
        glyphs: Vec::<Terminus16x18Glyph>::with_capacity(96),
    };
    for _i in 0..96 {
        font.glyphs.push(Terminus16x18Glyph {
            codepoint: 0,
            bitmap: [[0; FONT_WIDTH]; FONT_HEIGHT],
        });
    }

    let lines = &mut TERMINUS_U18N.lines();
    let mut glyph = &mut font.glyphs[0];
    // Current row of pixels.
    let mut row = 0 as usize;
    'lines: for (line_no, l) in lines.enumerate() {
        if line_no < 27 {
            continue 'lines;
        }
        let tokens = l.split(" ");
        'tokens: for t in tokens {
            match t {
                "STARTCHAR" => {
                    if !stack.is_empty() {
                        //TODO: propagate this error to caller using Result<T, U>
                        panic!("corrupt bdt: STARTCHAR")
                    }
                    row = 0;
                    stack.push(Token::STARTCHAR);
                    continue 'lines;
                }
                // ENCODING
                "ENCODING" => {
                    if *stack.last().unwrap_or(&Token::EMPTY) != Token::STARTCHAR {
                        //TODO: propagate this error to caller usint Result<T, U>
                        panic!("corrupt bdt: ENCODING")
                    }
                    stack.push(Token::ENCODING);
                    continue 'tokens;
                }
                "BITMAP" => {
                    let top = *stack.last().unwrap_or(&Token::EMPTY);
                    if top == Token::UNUSED {
                        continue 'lines;
                    }
                    if top != Token::STARTCHAR {
                        //TODO: propagate this error to caller usint Result<T, U>
                        panic!("corrupt bdt: BITMAP")
                    }
                    stack.push(Token::BITMAP);
                    continue 'lines;
                }
                "ENDCHAR" => {
                    let top = *stack.last().unwrap_or(&Token::EMPTY);
                    if top != Token::BITMAP && top != Token::UNUSED {
                        //TODO: propagate this error to caller usint Result<T, U>
                        panic!("corrupt bdt: ENDCHAR")
                    }
                    stack.pop(); // BITMAP || UNUSED
                    stack.pop(); // STARTCHAR
                    continue 'lines;
                }
                "ENDFONT" => {continue 'lines},
                v => match *stack.last().unwrap_or(&Token::EMPTY) {
                    Token::ENCODING => {
                        let parsed = v.parse::<u8>();
                        match parsed {
                            Ok(enc) => {
                                if enc > 127 || enc == 0{
                                    stack.pop();
                                    stack.push(Token::UNUSED);
                                    continue 'lines;
                                } else {
                                    glyph = &mut font.glyphs[enc as usize - 32];
                                }
                                glyph.codepoint = enc;
                                stack.pop();
                            },
                            Err(_) => {
                                stack.pop();
                                stack.push(Token::UNUSED);
                            },
                        }
                        continue 'lines;
                    },
                    Token::UNUSED => {
                        continue 'lines;
                    }
                    Token::BITMAP => {
                        let line_len = v.len();
                        let mut arr = Vec::<u8>::with_capacity(line_len);
                        let chars = v.chars();
                        for (i, c) in chars.enumerate() {
                            let d = c.to_digit(16).unwrap() as u8;
                            arr.push(d);

                            for (i, d) in arr.iter().enumerate() {
                                glyph.bitmap[row][line_len * i + 0] = (*d & 0b1000) >> 3;
                                glyph.bitmap[row][line_len * i + 1] = (*d & 0b100) >> 2;
                                glyph.bitmap[row][line_len * i + 2] = (*d & 0b10) >> 1;
                                glyph.bitmap[row][line_len * i + 3] = *d & 0b1;
                            }
                        }
                        row += 1;
                        continue 'lines;
                    }
                    Token::EMPTY => {
                        info!("v: {:?}, stack: {:?}", v, &stack);
                        panic!("corrupt bdt: value")
                    }
                    _ => {}
                },
            }
        }
    }
    return font;
}
