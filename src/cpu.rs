use std::{cell::RefCell, rc::Rc};

use crate::{mmu::Mmu, utils::Cycles};

pub(crate) struct Cpu {
    mmu: Rc<RefCell<Mmu>>,
}

impl Cpu {
    pub fn new(mmu: Rc<RefCell<Mmu>>) -> Self {
        Cpu { mmu }
    }

    pub fn execute_opcode(&mut self) -> Cycles {
        todo!()
    }
}
