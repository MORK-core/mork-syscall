use alloc::boxed::Box;
use mork_capability::cap::{CapType, ThreadCap};
use mork_capability::cnode::CapNode;
use mork_common::mork_kernel_log;
use mork_common::syscall::ipc_buffer::IPCBuffer;
use mork_common::syscall::message_info::{InvocationLabel, MessageInfo, ResponseLabel};
use mork_common::types::ResultWithErr;
use mork_hal::context::HALContextTrait;
use mork_task::task::TaskContext;
use mork_task::task_state::ThreadStateEnum;

pub fn handle(cspace: &CapNode, current: &TaskContext, dest_cap: ThreadCap, message_info: MessageInfo) -> ResultWithErr<MessageInfo> {
    match InvocationLabel::from_usize(message_info.get_label()) {
        InvocationLabel::TCBSuspend => {
            let task = TaskContext::from_cap(&dest_cap);
            task.state = ThreadStateEnum::ThreadStateInactive;
            Ok(())
        },
        InvocationLabel::TCBSetIPCBuffer => {
            let task = TaskContext::from_cap(&dest_cap);
            let frame_cap = cspace[current.hal_context.get_mr(0)];
            if frame_cap.get_type() != CapType::Frame {
                mork_kernel_log!(warn, "Invalid cap type: {:?}", frame_cap.get_type());
                return Err(MessageInfo::new_response(ResponseLabel::ErrCapType));
            }
            let ipc_buffer = unsafe {
                Box::from_raw((frame_cap.frame_cap.base_ptr() << 12) as *mut IPCBuffer)
            };
            task.ipc_buffer = Some(ipc_buffer);
            Ok(())
        }

        InvocationLabel::TCBSetTLSBase => {
            let task = TaskContext::from_cap(&dest_cap);
            let tls_base = current.hal_context.get_mr(0);
            task.hal_context.set_tls_base(tls_base);
            Ok(())
        }
        _ => {
            mork_kernel_log!(warn, "unSupported invocation label: {}", message_info.get_label());
            Err(MessageInfo::new_response(ResponseLabel::UnSupported))
        }
    }
}