use mork_capability::cap::ThreadCap;
use mork_common::mork_kernel_log;
use mork_common::syscall::message_info::{InvocationLabel, MessageInfo, ResponseLabel};
use mork_common::types::ResultWithErr;
use mork_task::task::TaskContext;
use mork_task::task_state::ThreadStateEnum;

pub fn handle(_current: &mut TaskContext, dest_cap: ThreadCap, message_info: MessageInfo) -> ResultWithErr<MessageInfo> {
    match InvocationLabel::from_usize(message_info.get_label()) {
        InvocationLabel::TCBSuspend => {
            let task = TaskContext::from_cap(&dest_cap);
            task.state = ThreadStateEnum::ThreadStateInactive;
            Ok(())
        },
        _ => {
            mork_kernel_log!(warn, "unSupported invocation label: {}", message_info.get_label());
            Err(MessageInfo::new_response(ResponseLabel::UnSupported))
        }
    }
}