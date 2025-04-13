use alloc::boxed::Box;
use mork_capability::cap::{CapType, ThreadCap};
use mork_common::constants::CNodeSlot;
use mork_common::hal::{UserContext, UserContextTrait, MAX_GENERAL_REGISTER_NUM};
use mork_common::mork_kernel_log;
use mork_common::syscall::message_info::{InvocationLabel, MessageInfo, ResponseLabel};
use mork_common::types::ResultWithErr;
use mork_hal::context::HALContextTrait;
use mork_kernel_state::KernelSafeAccessData;
use mork_task::task::TaskContext;
use mork_task::task_state::ThreadStateEnum;

pub fn handle(kernel_state: &mut KernelSafeAccessData, current: &mut TaskContext,
              dest_cap: ThreadCap, message_info: MessageInfo)
              -> ResultWithErr<MessageInfo> {
    let task = TaskContext::from_cap(&dest_cap);
    match InvocationLabel::from_usize(message_info.get_label()) {
        InvocationLabel::TCBSuspend => {
            task.state = ThreadStateEnum::ThreadStateInactive;
            Ok(())
        },
        InvocationLabel::TCBResume => {
            if !task.is_queued {
                task.state = ThreadStateEnum::ThreadStateRestart;
                if task.get_ptr() != current.get_ptr() {
                    unsafe {
                        kernel_state.scheduler.enqueue_back(Box::from_raw(task as *mut TaskContext));
                    }
                    task.is_queued = true;
                }
            }
            Ok(())
        }

        InvocationLabel::TCBSetIPCBuffer => {
            let cspace = current.cspace.as_ref().unwrap();
            let frame_cap = cspace[current.hal_context.get_mr(0)];
            if frame_cap.get_type() != CapType::Frame {
                mork_kernel_log!(warn, "Invalid cap type: {:?}", frame_cap.get_type());
                return Err(MessageInfo::new_response(ResponseLabel::ErrCapType));
            }
            if task.get_ptr() == current.get_ptr() {
                task.ipc_buffer = Some(current.hal_context.get_mr(0));
            } else {
                let frame_cap_new = frame_cap.derive();
                let target_cspace = task.cspace.as_mut().unwrap();
                if let Some(index) = target_cspace.alloc_free() {
                    target_cspace[index] = frame_cap_new;
                    task.ipc_buffer = Some(index);
                }
            }
            Ok(())
        }

        InvocationLabel::TCBSetSpace => {
            handle_set_space(task, current)
        }

        InvocationLabel::TCBSetTLSBase => {
            let tls_base = current.hal_context.get_mr(0);
            task.hal_context.set_tls_base(tls_base);
            Ok(())
        }

        InvocationLabel::TCBReadRegisters => {
            handle_read_registers(task, current)
        }

        InvocationLabel::TCBWriteRegisters => {
            handle_write_registers(task, current)
        }

        _ => {
            mork_kernel_log!(warn, "unSupported invocation label: {}", message_info.get_label());
            Err(MessageInfo::new_response(ResponseLabel::UnSupported))
        }
    }
}

fn handle_read_registers(task: &mut TaskContext, current: &mut TaskContext) ->  ResultWithErr<MessageInfo> {
    if let Some(buffer) = current.get_ipc_buffer_mut() {
        let user_context = UserContext::from_ipc_buffer_mut(buffer);
        for i in 0..MAX_GENERAL_REGISTER_NUM {
            user_context[i] = task.hal_context[i];
        }
        user_context.set_next_ip(task.hal_context.get_next_ip());
        Ok(())
    } else {
        mork_kernel_log!(warn, "No IPC buffer available");
        Err(MessageInfo::new_response(ResponseLabel::NoIpcBuffer))
    }
}

fn handle_write_registers(task: &mut TaskContext, current: &mut TaskContext) -> ResultWithErr<MessageInfo> {
    if let Some(buffer) = current.get_ipc_buffer() {
        let user_context = UserContext::from_ipc_buffer(buffer);
        for i in 0..MAX_GENERAL_REGISTER_NUM {
            task.hal_context[i] = user_context[i];
        }
        task.hal_context.set_next_ip(user_context.get_next_ip());
        Ok(())
    } else {
        mork_kernel_log!(warn, "No IPC buffer available");
        Err(MessageInfo::new_response(ResponseLabel::NoIpcBuffer))
    }
}

fn handle_set_space(task: &mut TaskContext, current: &mut TaskContext) -> ResultWithErr<MessageInfo> {
    let is_current = task.get_ptr() == current.get_ptr();
    let cspace = current.cspace.as_mut().unwrap();
    let vspace_cap = cspace[current.hal_context.get_mr(0)];
    if vspace_cap.get_type() != CapType::PageTable {
        mork_kernel_log!(warn, "Invalid cap type: {:?}", vspace_cap.get_type());
        return Err(MessageInfo::new_response(ResponseLabel::ErrCapType));
    }
    if is_current {
        if current.hal_context.get_mr(0) != CNodeSlot::CapInitVSpace as usize {
            cspace.free_slot(CNodeSlot::CapInitVSpace as usize);
            cspace[CNodeSlot::CapInitVSpace as usize] = vspace_cap.derive();
        }
    } else {
        let target_cspace = task.cspace.as_mut().unwrap();
        target_cspace.free_slot(CNodeSlot::CapInitVSpace as usize);
        target_cspace[CNodeSlot::CapInitVSpace as usize] = vspace_cap.derive();
    }
    Ok(())

}