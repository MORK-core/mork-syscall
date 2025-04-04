#![no_std]
extern crate alloc;

use mork_capability::cap::CapType;
use mork_common::constants::MAX_CNODE_SIZE;
use mork_common::mork_kernel_log;
use mork_common::syscall::message_info::{MessageInfo, ResponseLabel};
use mork_common::syscall::Syscall;
use mork_hal::context::HALContextTrait;
use mork_kernel_state::{KernelSafeAccessData};
use mork_task::task_state::ThreadStateEnum;

mod other;
mod invocation;

pub fn handle_syscall(kernel_state: &mut KernelSafeAccessData,
                      _cptr: usize, _msg_info: MessageInfo, syscall: Syscall) {
    let mut current = kernel_state.current_task.take().unwrap();
    match syscall {
        Syscall::SysDebugPutChar => {
            mork_hal::console_putchar(char::from(current.hal_context.get_cap() as u8));
        }
        Syscall::Syscall => {
            let mut response = MessageInfo::new_response(ResponseLabel::Success);
            let dest_cap_idx = current.hal_context.get_cap();
            if dest_cap_idx >= MAX_CNODE_SIZE {
                mork_kernel_log!(warn, "invalid dest cap: {}", dest_cap_idx);
                response = MessageInfo::new_response(ResponseLabel::OutOfRange);
            } else if let Some(cspace) = current.cspace.as_ref() {
                let dest_cap = cspace.get()[dest_cap_idx];
                let message_tag = current.hal_context.get_tag();
                match dest_cap.get_type() {
                    CapType::CNode => {
                        match invocation::cspace_handler::handle(
                            &mut current, unsafe { dest_cap.cnode_cap }, message_tag)
                        {
                            Ok(res) => {
                                current.hal_context.set_mr(0, res);
                            }
                            Err(resp) => {
                                response = resp;
                            }
                        }
                    }

                    CapType::Thread => {
                        match invocation::task_handler::handle(cspace.get(),
                            &current, unsafe { dest_cap.thread_cap }, message_tag
                        ) {
                            Ok(_) => {}
                            Err(_) => {}
                        }
                    }

                    CapType::PageTable => {
                        match invocation::memory_handler::handle(
                            cspace.get_mut(), &current, unsafe { dest_cap.page_table_cap }, message_tag
                        ) {
                            Ok(_) => {}
                            Err(resp) => {
                                response = resp;
                            }
                        }
                    }
                    _ => {
                        mork_kernel_log!(warn, "unSupported cap type: {:?}", dest_cap.get_type());
                        response = MessageInfo::new_response(ResponseLabel::UnSupported);
                    }
                }
            } else {
                mork_kernel_log!(warn, "try to find cspace failed");
                response = MessageInfo::new_response(ResponseLabel::NotEnoughSpace);
            }
            current.hal_context.set_tag(response);
        }
        _ => {
            panic!("Unsupported syscall type: {:?}", syscall);
        }
    }
    if current.state == ThreadStateEnum::ThreadStateRestart {
        kernel_state.scheduler.enqueue(current);
    }
}