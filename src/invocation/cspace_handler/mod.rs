mod alloc_handler;

use mork_capability::cap::CNodeCap;
use mork_capability::cnode::CapNode;
use mork_common::constants::ObjectType;
use mork_common::mork_kernel_log;
use mork_common::syscall::message_info::{InvocationLabel, MessageInfo, ResponseLabel};
use mork_hal::context::HALContextTrait;
use mork_task::task::TaskContext;
use crate::invocation::cspace_handler::alloc_handler::AllocHandler;

pub fn handle(current: &mut TaskContext, dest_cap: CNodeCap, message_info: MessageInfo) -> Result<usize, MessageInfo> {
    match InvocationLabel::from_usize(message_info.get_label()) {
        InvocationLabel::AllocObject => {
            let cspace = CapNode::from_cap(&dest_cap);
            let mut handler = AllocHandler { cspace };
            let obj_type = ObjectType::from_usize(current.hal_context.get_mr(0));
            handler.handle(obj_type)
        }
        _ => {
            mork_kernel_log!(warn, "unSupported invocation label: {}", message_info.get_label());
            Err(MessageInfo::new_response(ResponseLabel::UnSupported))
        }
    }
}