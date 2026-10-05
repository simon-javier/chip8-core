use rand::random;

const FONTSET_SIZE: usize = 80;
const FONTSET: [u8; FONTSET_SIZE] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80  // F
];

pub const SCREEN_WIDTH: usize = 64;
pub const SCREEN_HEIGHT: usize = 32;

const RAM_SIZE: usize = 4096;
const NUM_REGS: usize = 16;
const STACK_SIZE: usize = 16;
const NUM_KEYS: usize = 16;

const START_ADDR: u16 = 0x200;

pub struct Emu {
    pc: u16,
    ram: [u8; RAM_SIZE],
    screen: [bool; SCREEN_HEIGHT * SCREEN_WIDTH],
    v_reg: [u8; NUM_REGS],
    i_reg: u16,
    sp: u16,
    stack: [u16; STACK_SIZE],
    keys: [bool; NUM_KEYS],
    dt: u8,
    st: u8,
    pub waiting_vblank: bool,
}

impl Emu {
    pub fn new() -> Self {
        let mut new_emu = Self {
            pc: START_ADDR,
            ram: [0; RAM_SIZE],
            screen: [false; SCREEN_HEIGHT * SCREEN_WIDTH],
            v_reg: [0; NUM_REGS],
            i_reg: 0,
            sp: 0,
            stack: [0; STACK_SIZE],
            keys: [false; NUM_KEYS],
            dt: 0,
            st: 0,
            waiting_vblank: false,
        };
        new_emu.ram[..FONTSET_SIZE].copy_from_slice(&FONTSET);
        new_emu
    }

    pub fn get_display(&self) -> &[bool] {
        &self.screen
    }

    pub fn keypress(&mut self, idx: usize, pressed: bool) {
        self.keys[idx] = pressed;
    }

    pub fn load(&mut self, data: &[u8]) {
        let start = START_ADDR as usize;
        let end = (START_ADDR as usize) + data.len();
        self.ram[start..end].copy_from_slice(data);
    }

    pub fn reset(&mut self) {
        self.pc = START_ADDR;
        self.ram = [0; RAM_SIZE];
        self.screen = [false; SCREEN_HEIGHT * SCREEN_WIDTH];
        self.v_reg = [0; NUM_REGS];
        self.i_reg = 0;
        self.sp = 0;
        self.stack = [0; STACK_SIZE];
        self.keys = [false; NUM_KEYS];
        self.dt = 0;
        self.st = 0;
        self.ram[..FONTSET_SIZE].copy_from_slice(&FONTSET);
        self.waiting_vblank = false;
    }

    fn push(&mut self, val: u16) {
        self.stack[self.sp as usize] = val;
        self.sp += 1;
    }

    fn pop(&mut self) -> u16 {
        self.sp -= 1;
        self.stack[self.sp as usize]
    }

    pub fn tick(&mut self) {
        if self.waiting_vblank {
            return;
        }
        //Fetch
        let op = self.fetch();

        //Decode & Execute
        self.execute(op)

    }

    fn execute(&mut self, op: u16) {
        let digit1 = ((op & 0xF000) >> 12) as u8;
        let digit2 = ((op & 0x0F00) >> 8) as u8;
        let digit3 = ((op & 0x00F0) >> 4) as u8;
        let digit4 = (op & 0x000F) as u8;
        match (digit1, digit2, digit3, digit4) {
            (0,0,0,0) => return,                                                    // NOP
            (0,0,0xE,0) => self.screen = [false; SCREEN_HEIGHT * SCREEN_WIDTH],     // Clear Screen
            (1,_,_,_) => self.pc = op & 0x0FFF,                                     // Jump
            (6,_,_,_) => self.v_reg[digit2 as usize] = (op & 0x00FF) as u8,         // Set
            (7,_,_,_) => self.add_nn(digit2 as usize, (op & 0x00FF) as u8),         // Add
            (0xA,_,_,_) => self.i_reg = op & 0x0FFF,                                // Set Index
            (0xD,_,_,_) => self.display(digit2, digit3, digit4),                    // Display
            (2,_,_,_) => { self.push(self.pc); self.pc = op & 0x0FFF },             // Subroutine Push
            (0,0,0xE,0xE) => self.pc = self.pop(),                                  // Subroutine Pop
            (3,_,_,_) => {                                                          // Skip Conditionally
                let x = digit2 as usize;
                if self.v_reg[x] == (op & 0xFF) as u8 { self.pc += 2; }
            },
            (4,_,_,_) => {                                                          // Skip Conditionally
                let x = digit2 as usize;
                if self.v_reg[x] != (op & 0xFF) as u8 { self.pc += 2; }
            },
            (5,_,_,0) => {                                                          // Skip Conditionally
                let x = digit2 as usize;
                let y = digit3 as usize;
                if self.v_reg[x] == self.v_reg[y] { self.pc += 2; }
            },
            (9,_,_,0) => {                                                          // Skip Conditionally
                let x = digit2 as usize;
                let y = digit3 as usize;
                if self.v_reg[x] != self.v_reg[y] { self.pc += 2; }
            },
            (8,_,_,_) => {
                let x = digit2 as usize;
                let y = digit3 as usize;
                match digit4 {
                    0 => self.v_reg[x] = self.v_reg[y],
                    1 => {
                        self.v_reg[0xF] = 0;
                        self.v_reg[x] |= self.v_reg[y];
                    },
                    2 => {
                        self.v_reg[0xF] = 0;
                        self.v_reg[x] &= self.v_reg[y];
                    },
                    3 => {
                        self.v_reg[0xF] = 0;
                        self.v_reg[x] ^= self.v_reg[y];
                    },
                    4 => {
                        let (new_vx, carry) = self.v_reg[x].overflowing_add(self.v_reg[y]);
                        let new_vf = if carry { 1 } else { 0 };
                        self.v_reg[x] = new_vx;
                        self.v_reg[0xF] = new_vf;
                    },
                    5 => {
                        let (new_vx, borrow) = self.v_reg[x].overflowing_sub(self.v_reg[y]);
                        let new_vf = if !borrow { 1 } else { 0 };
                        self.v_reg[x] = new_vx;
                        self.v_reg[0xF] = new_vf;
                    },
                    7 => {
                        let (new_vx, borrow) = self.v_reg[y].overflowing_sub(self.v_reg[x]);
                        let new_vf = if !borrow { 1 } else { 0 };
                        self.v_reg[x] = new_vx;
                        self.v_reg[0xF] = new_vf;
                    },
                    6 => {
                        self.v_reg[x] = self.v_reg[y];
                        let lsb = self.v_reg[x] & 1;
                        self.v_reg[x] >>= 1;
                        self.v_reg[0xF] = lsb;
                    },
                    0xE => {
                        self.v_reg[x] = self.v_reg[y];
                        let msb = (self.v_reg[x] >> 7) & 1;
                        self.v_reg[x] <<= 1;
                        self.v_reg[0xF] = msb;
                    },
                    _ => unimplemented!("Unimplemented opcode: {}", op),

                }
            },
            (0xB,_,_,_) => self.pc = (0xFFF & op) + self.v_reg[0] as u16,
            (0xC,_,_,_) => {
                let x = digit2 as usize;
                let nn = (op & 0xFF) as u8;
                let rng: u8 = random();

                self.v_reg[x] = nn & rng;
            },
            (0xE,_,9,0xE) => {
                let x = digit2 as usize;
                let vx_val = self.v_reg[x] as usize;
                if self.keys[vx_val] { self.pc += 2 }
            },
            (0xE,_,0xA,1) => {
                let x = digit2 as usize;
                let vx_val = self.v_reg[x] as usize;
                if !self.keys[vx_val] { self.pc += 2 }
            },
            // Timer opcodes
            (0xF,_,0,7) => self.v_reg[digit2 as usize] = self.dt,
            (0xF,_,1,5) => self.dt = self.v_reg[digit2 as usize],
            (0xF,_,1,8) => self.st = self.v_reg[digit2 as usize],

            (0xF,_,1,0xE) => {
                let x = digit2 as usize;
                self.i_reg = self.i_reg.wrapping_add(self.v_reg[x] as u16);
            },
            (0xF,_,0,0xA) => {
                let x = digit2 as usize;
                let mut pressed = false;
                for i in 0..self.keys.len() {
                    if self.keys[i] {
                        self.v_reg[x] = i as u8;
                        pressed = true;
                        break;
                    }
                }
                if !pressed {
                    self.pc -= 2;
                }
            },
            (0xF,_,2,9) => {
                let x = digit2 as usize;
                let c = self.v_reg[x] as u16;
                self.i_reg = c * 5;
            }
            (0xF,_,3,3) => {
                let x = digit2 as usize;
                let vx = self.v_reg[x];
                let hundreds = vx / 100;
                let tens = (vx / 10) % 10;
                let ones = vx % 10;

                self.ram[self.i_reg as usize] = hundreds;
                self.ram[(self.i_reg + 1) as usize] = tens;
                self.ram[(self.i_reg + 2) as usize] = ones;
            },
            (0xF,_,5,5) => {
                let x = digit2 as usize;
                let i = self.i_reg as usize;
                for idx in 0..=x {
                    self.ram[i + idx] = self.v_reg[idx];
                }
                self.i_reg += (x + 1) as u16;
            },
            (0xF,_,6,5) => {
                let x = digit2 as usize;
                let i = self.i_reg as usize;
                for idx in 0..=x {
                    self.v_reg[idx] = self.ram[i + idx];
                }
                self.i_reg += (x + 1) as u16;
            },
            (_,_,_,_) => unimplemented!("Unimplemented opcode: {}", op),
        }
    }

    fn display(&mut self, vx_val:u8, vy_val:u8, n:u8) {
        let x_coord = (self.v_reg[vx_val as usize] as usize) % SCREEN_WIDTH;
        let y_coord = (self.v_reg[vy_val as usize] as usize) % SCREEN_HEIGHT;

        let mut flipped = false;

        for y_line in 0..(n as usize) {
            let y = y_coord + y_line;

            if y >= SCREEN_HEIGHT {
                break;
            }

            let addr = self.i_reg + y_line as u16;
            let pixels = self.ram[addr as usize];

            for x_line in 0..8 {
                let x = x_coord + x_line;

                if x >= SCREEN_WIDTH {
                    break;
                }

                if (pixels & (0b1000_0000 >> x_line)) != 0 {
                    let idx = x + SCREEN_WIDTH * y;

                    flipped |= self.screen[idx];
                    self.screen[idx] ^= true;
                }
            }
        }
        self.waiting_vblank = true;
        self.v_reg[0xF] = flipped as u8;
    }

    fn add_nn(&mut self, x: usize, val: u8) {
        self.v_reg[x] = self.v_reg[x].wrapping_add(val);
    }

    fn fetch(&mut self) -> u16 {
        let higher_byte = self.ram[self.pc as usize] as u16;
        let lower_byte = self.ram[(self.pc + 1) as usize] as u16;

        let op = (higher_byte << 8) | lower_byte;
        self.pc += 2;
        op
    }

    pub fn tick_timers(&mut self) {
        if self.dt > 0 {
            self.dt -= 1;
        }

        if self.st > 0 {
            if self.st == 1 {
                // BEEP
            }
            self.st -= 1;
        }
        self.waiting_vblank = false;
    }
}
