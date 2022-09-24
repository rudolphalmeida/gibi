use crate::interrupts::{InterruptHandler, InterruptType};
use crate::memory::Memory;
use crate::utils::{Byte, Word};
use std::cell::RefCell;
use std::rc::Rc;

pub const TIMER_START: Word = 0xFF04;
pub const TIMER_END: Word = 0xFF07;

const DIV_ADDRESS: Word = 0xFF04;
const TIMA_ADDRESS: Word = 0xFF05;
const TMA_ADDRESS: Word = 0xFF06;
const TIMER_CONTROL: Word = 0xFF07;

pub(crate) struct Timer {
    div: Word,
    tima: Byte,
    tma: Byte,
    tac: Byte,

    previous_tima_inc_result: bool,
    tima_overflowed_last_cycle: bool,

    interrupts: Rc<RefCell<InterruptHandler>>,
}

impl Timer {
    pub fn new(interrupts: Rc<RefCell<InterruptHandler>>) -> Self {
        Timer {
            div: 0x0000,
            tima: 0x00,
            tma: 0x00,
            tac: 0x00,

            previous_tima_inc_result: false,
            tima_overflowed_last_cycle: false,

            interrupts,
        }
    }

    pub fn tick(&mut self) {
        if self.tima_overflowed_last_cycle {
            self.tima = self.tma;

            self.interrupts
                .borrow_mut()
                .request_interrupt(InterruptType::Timer);

            self.tima_overflowed_last_cycle = false;
        }

        for _ in 0..4 {
            self.div = self.div.wrapping_add(1);

            let tima_increment_bit = self.div & tima_bit_position(self.tac) != 0;
            let timer_enabled_bit = self.tac & 0b100 != 0;

            let tima_inc_result = tima_increment_bit && timer_enabled_bit;
            // Check for falling edge
            if self.previous_tima_inc_result && !tima_inc_result {
                let (inc_tima, overflow) = self.tima.overflowing_add(1);
                self.tima = inc_tima;
                if overflow {
                    self.tima_overflowed_last_cycle = true;
                }
            }

            self.previous_tima_inc_result = tima_inc_result;
        }
    }
}

impl Memory for Timer {
    fn read(&self, address: Word) -> Byte {
        match address {
            DIV_ADDRESS => (self.div >> 8) as Byte,
            TIMA_ADDRESS => self.tima,
            TMA_ADDRESS => self.tma,
            TIMER_CONTROL => self.tac,
            _ => panic!("Invalid address {:#6X} to Timer::read", address),
        }
    }

    fn write(&mut self, address: Word, data: Byte) {
        match address {
            DIV_ADDRESS => self.div = 0x0000,
            TIMA_ADDRESS => self.tima = data,
            TMA_ADDRESS => self.tma = data,
            TIMER_CONTROL => self.tac = data & 0b111,
            _ => panic!("Invalid address {:#6X} to Timer::write", address),
        }
    }
}

fn tima_bit_position(tac: Byte) -> Word {
    match tac & 0b11 {
        0 => 1 << 9,
        1 => 1 << 3,
        2 => 1 << 5,
        3 => 1 << 7,
        _ => panic!("This is not possible"),
    }
}
