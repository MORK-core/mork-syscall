#![no_std]
extern crate alloc;

use mork_common::syscall::message_info::MessageInfo;
use mork_common::syscall::Syscall;
use mork_hal::context::HALContextTrait;
use mork_kernel_state::{KernelSafeAccessData};
use mork_task::task_state::ThreadStateEnum;

mod other;
mod invocation;

pub fn handle_syscall(kernel_state: &mut KernelSafeAccessData,
                      _cptr: usize, _msg_info: MessageInfo, syscall: Syscall) {
    let current = kernel_state.current_task.take().unwrap();
    match syscall {
        Syscall::SysDebugPutChar => {
            mork_hal::console_putchar(char::from(current.hal_context.get_cap() as u8));
        }
        _ => {
            panic!("Unsupported syscall type: {:?}", syscall);
        }
    }

    if current.state == ThreadStateEnum::ThreadStateRestart {
        kernel_state.scheduler.enqueue(current);
    }
}