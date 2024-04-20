use crate::interrupts::{InterruptHandler, InterruptType};
use crate::memory::Memory;
use crate::{ExecutionState, SystemState};
use std::cell::RefCell;
use std::rc::Rc;

pub const TIMER_START: u16 = 0xFF04;
pub const TIMER_END: u16 = 0xFF07;

const DIV_ADDRESS: u16 = 0xFF04;
const TIMA_ADDRESS: u16 = 0xFF05;
const TMA_ADDRESS: u16 = 0xFF06;
const TIMER_CONTROL: u16 = 0xFF07;

pub(crate) struct Timer {
    div: u16,
    tima: u8,
    tma: u8,
    tac: u8,

    previous_tima_inc_result: bool,
    /// The TIMA overflow reset with TMA is delayed by one m-cycle or 4 t-cycles.
    /// Since we tick the timer one m-cycle at a time we keep track of which
    /// t-cycle within that m-cycle TIMA overflowed and exactly 4 t-cycles later
    /// reset it with TMA
    tima_overflowed_last_cycle: Option<i32>,

    interrupts: Rc<RefCell<InterruptHandler>>,
}

impl Timer {
    pub fn new(
        interrupts: Rc<RefCell<InterruptHandler>>,
    ) -> Self {
        Timer {
            div: 0x0000,
            tima: 0x00,
            tma: 0x00,
            tac: 0x00,

            previous_tima_inc_result: false,
            tima_overflowed_last_cycle: None,

            interrupts,
        }
    }

    pub fn tick(&mut self, system_state: &mut SystemState) {
        let prev_i = if let Some(i) = self.tima_overflowed_last_cycle {
            i
        } else {
            -1
        };

        for i in 0..4 {
            self.div = self.div.wrapping_add(1);
            if self.div == 0x0000
                && system_state.execution_state
                    == ExecutionState::PreparingSpeedSwitch
            {
                // DIV overflowed. Complete speed switch
                system_state.key1 ^= 0x81; // Toggle speed and reset switch request
                system_state.execution_state = ExecutionState::ExecutingProgram;
            }

            // Reset TIMA if it overflowed exactly 4 t-cycles ago
            if prev_i == i {
                self.tima = self.tma;

                self.interrupts
                    .borrow_mut()
                    .request_interrupt(InterruptType::Timer);

                self.tima_overflowed_last_cycle = None;
            }

            let tima_increment_bit = self.div & tima_bit_position(self.tac) != 0;
            let timer_enabled_bit = self.tac & 0b100 != 0;

            let tima_inc_result = tima_increment_bit && timer_enabled_bit;
            // Check for falling edge
            if self.previous_tima_inc_result && !tima_inc_result {
                let (inc_tima, overflow) = self.tima.overflowing_add(1);
                self.tima = inc_tima;
                if overflow {
                    self.tima_overflowed_last_cycle = Some(i);
                    // TIMA reads 0x00 for the 4 t-cycles before it is reset
                    self.tima = 0x00;
                }
            }

            self.previous_tima_inc_result = tima_inc_result;
        }
    }
}

impl Memory for Timer {
    fn read(&mut self, address: u16) -> u8 {
        match address {
            DIV_ADDRESS => (self.div >> 8) as u8,
            TIMA_ADDRESS => self.tima,
            TMA_ADDRESS => self.tma,
            TIMER_CONTROL => self.tac,
            _ => panic!("Invalid address {:#6X} to Timer::read", address),
        }
    }

    fn write(&mut self, address: u16, data: u8) {
        match address {
            DIV_ADDRESS => self.div = 0x0000,
            TIMA_ADDRESS => self.tima = data,
            TMA_ADDRESS => self.tma = data,
            TIMER_CONTROL => self.tac = data & 0b111,
            _ => panic!("Invalid address {:#6X} to Timer::write", address),
        }
    }
}

fn tima_bit_position(tac: u8) -> u16 {
    match tac & 0b11 {
        0 => 1 << 9,
        1 => 1 << 3,
        2 => 1 << 5,
        3 => 1 << 7,
        _ => panic!("This is not possible"),
    }
}
