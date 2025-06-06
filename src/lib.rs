#![no_std]
extern crate alloc;

use alloc::boxed::Box;
use mork_capability::cap::CapType;
use mork_common::constants::MAX_CNODE_SIZE;
use mork_common::mork_kernel_log;
use mork_common::syscall::message_info::{InvocationLabel, MessageInfo, ResponseLabel};
use mork_common::syscall::Syscall;
use mork_hal::context::HALContextTrait;
use mork_kernel_state::{KernelSafeAccessData};
use mork_task::task_state::ThreadStateEnum;

mod other;
mod invocation;

pub use invocation::cspace_handler::DeallocHandler;
use mork_task::task::TaskContext;
use mork_ipc::notification::Notification;

pub fn handle_syscall(kernel_state: &mut KernelSafeAccessData,
                      _cptr: usize, _msg_info: MessageInfo, syscall: Syscall) {
    let mut current = kernel_state.current_task.take().unwrap();
    if current.state == ThreadStateEnum::ThreadStateRunning {
        current.state = ThreadStateEnum::ThreadStateRestart;
    }

    match syscall {
        Syscall::SysDebugPutChar => {
            mork_hal::console_putchar(char::from(current.hal_context.get_cap() as u8));
        }

        Syscall::SysDebugShutdown => {
            mork_hal::shutdown(false);
        }
        Syscall::Syscall => {
            mork_kernel_log!(debug, "start syscall: {:?}", InvocationLabel::from_usize(_msg_info.get_label()));
            handle_call(kernel_state, &mut current);
        }
        Syscall::SysNBSend => {
            handle_nb_send(kernel_state, &mut current);
        }
        Syscall::SysRecv => {
            handle_recv(&mut current);
        }
        _ => {
            panic!("Unsupported syscall type: {:?}", syscall);
        }
    }
    if current.state == ThreadStateEnum::ThreadStateRestart {
        kernel_state.scheduler.enqueue_front(current);
    } else {
        current.is_queued = false;
        Box::leak(current);
    }
}

fn handle_call(kernel_state: &mut KernelSafeAccessData, current: &mut TaskContext) {
    let mut response = MessageInfo::new_response(ResponseLabel::Success);
    let dest_cap_idx = current.hal_context.get_cap();
    if dest_cap_idx >= MAX_CNODE_SIZE {
        mork_kernel_log!(warn, "invalid dest cap: {}", dest_cap_idx);
        response = MessageInfo::new_response(ResponseLabel::OutOfRange);
    } else if let Some(cspace) = current.cspace.as_ref() {
        let dest_cap = cspace[dest_cap_idx];
        let message_tag = current.hal_context.get_tag();
        match dest_cap.get_type() {
            CapType::Thread => {
                match invocation::task_handler::handle(
                    kernel_state,
                    current,
                    unsafe { dest_cap.thread_cap },
                    message_tag
                ) {
                    Ok(res) => {
                        current.hal_context.set_mr(0, res);
                    }
                    Err(resp) => {
                        response = resp;
                    }
                }
            }

            CapType::PageTable => {
                match invocation::memory_handler::handle(
                    current, unsafe { dest_cap.page_table_cap }, message_tag
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

fn handle_nb_send(kernel_state: &mut KernelSafeAccessData, current: &mut TaskContext) {
    let mut response = MessageInfo::new_response(ResponseLabel::Success);
    let dest_cap_idx = current.hal_context.get_cap();
    if dest_cap_idx >= MAX_CNODE_SIZE {
        mork_kernel_log!(warn, "invalid dest cap: {}", dest_cap_idx);
        response = MessageInfo::new_response(ResponseLabel::OutOfRange);
    } else if let Some(cspace) = current.cspace.as_ref() {
        let dest_cap = cspace[dest_cap_idx];
        match dest_cap.get_type() {
            CapType::Notification => {
                let notification_cap = unsafe { dest_cap.notification_cap };
                let notification = Notification::from_cap(&notification_cap);
                let badge = notification_cap.badge() as usize;
                if let Some(task) = notification.signal(badge) {
                    kernel_state.scheduler.enqueue_back(task);
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

fn handle_recv(current: &mut TaskContext) {
    let mut response = MessageInfo::new_response(ResponseLabel::Success);
    let dest_cap_idx = current.hal_context.get_cap();
    if dest_cap_idx >= MAX_CNODE_SIZE {
        mork_kernel_log!(warn, "invalid dest cap: {}", dest_cap_idx);
        response = MessageInfo::new_response(ResponseLabel::OutOfRange);
    } else if let Some(cspace) = current.cspace.as_ref() {
        let dest_cap = cspace[dest_cap_idx];
        match dest_cap.get_type() {
            CapType::Notification => {
                let notification_cap = unsafe { dest_cap.notification_cap };
                let notification = Notification::from_cap(&notification_cap);
                notification.receive(current);
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