use alloc::boxed::Box;
use mork_capability::cap::{CapType, ThreadCap};
use mork_common::constants::{CNodeSlot, PAGE_SIZE_NORMAL};
use mork_common::hal::{UserContext, UserContextTrait, MAX_GENERAL_REGISTER_NUM};
use mork_common::mork_kernel_log;
use mork_common::syscall::message_info::{InvocationLabel, MessageInfo, ResponseLabel};
use mork_common::utils::alignas::is_aligned;
use mork_hal::context::HALContextTrait;
use mork_kernel_state::KernelSafeAccessData;
use mork_mm::page_table::{map_kernel_window, PageTableWrapper, PageTable};
use mork_task::task::TaskContext;
use mork_task::task_state::ThreadStateEnum;

pub fn handle(kernel_state: &mut KernelSafeAccessData, current: &mut TaskContext,
              dest_cap: ThreadCap, message_info: MessageInfo)
              -> Result<usize, MessageInfo> {
    let task = TaskContext::from_cap(&dest_cap);
    if message_info.get_label() >= InvocationLabel::CNodeAlloc as usize
        && message_info.get_label() <= InvocationLabel::CNodeSaveCaller as usize {
        return super::cspace_handler::handle(current, dest_cap, message_info);
    }
    match InvocationLabel::from_usize(message_info.get_label()) {
        InvocationLabel::TCBSuspend => {
            task.state = ThreadStateEnum::ThreadStateInactive;
            Ok(0)
        },
        InvocationLabel::TCBResume => {
            if !task.is_queued {
                task.state = ThreadStateEnum::ThreadStateRestart;
                if task.get_ptr() != current.get_ptr() {
                    unsafe {
                        mork_kernel_log!(debug, "task enqueue");
                        kernel_state.scheduler.enqueue_back(Box::from_raw(task as *mut TaskContext));
                    }
                    task.is_queued = true;
                }
            }
            Ok(0)
        }

        InvocationLabel::TCBSetIPCBuffer => {
            let vaddr = current.hal_context.get_mr(0);
            if !is_aligned(vaddr, PAGE_SIZE_NORMAL) {
                mork_kernel_log!(warn, "Invalid vaddr {:#x}", vaddr);
                return Err(MessageInfo::new_response(ResponseLabel::InvalidParam));
            }
            let vspace = current.get_vspace_mut().unwrap();
            let wrapper = PageTableWrapper::new(vspace);
            if let Some(ipc_buffer_ptr) = wrapper.va_to_pa(vaddr) {
                task.ipc_buffer_ptr = Some(ipc_buffer_ptr)
            }

            Ok(0)
        }

        InvocationLabel::TCBSetSpace => {
            handle_set_space(task, current)
        }

        InvocationLabel::TCBSetTLSBase => {
            let tls_base = current.hal_context.get_mr(0);
            task.hal_context.set_tls_base(tls_base);
            Ok(0)
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

fn handle_read_registers(task: &mut TaskContext, current: &mut TaskContext) ->  Result<usize, MessageInfo> {
    if let Some(buffer) = current.get_ipc_buffer_mut() {
        let user_context = UserContext::from_ipc_buffer_mut(buffer);
        for i in 0..MAX_GENERAL_REGISTER_NUM {
            user_context[i] = task.hal_context[i];
        }
        user_context.set_next_ip(task.hal_context.get_next_ip());
        Ok(0)
    } else {
        mork_kernel_log!(warn, "No IPC buffer available");
        Err(MessageInfo::new_response(ResponseLabel::NoIpcBuffer))
    }
}

fn handle_write_registers(task: &mut TaskContext, current: &mut TaskContext) -> Result<usize, MessageInfo> {
    if let Some(buffer) = current.get_ipc_buffer() {
        let user_context = UserContext::from_ipc_buffer(buffer);
        for i in 0..MAX_GENERAL_REGISTER_NUM {
            task.hal_context[i] = user_context[i];
        }
        task.hal_context.set_next_ip(user_context.get_next_ip());
        Ok(0)
    } else {
        mork_kernel_log!(warn, "No IPC buffer available");
        Err(MessageInfo::new_response(ResponseLabel::NoIpcBuffer))
    }
}

fn handle_set_space(task: &mut TaskContext, current: &mut TaskContext) -> Result<usize, MessageInfo> {
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
        let page_table_cap = unsafe { vspace_cap.page_table_cap};
        let page_table = PageTable::from_cap(&page_table_cap );
        map_kernel_window(page_table).unwrap();
        target_cspace[CNodeSlot::CapInitVSpace as usize] = vspace_cap.derive();
    }
    Ok(0)
}