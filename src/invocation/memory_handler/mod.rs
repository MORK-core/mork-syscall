use mork_capability::cap::{CapType, PageTableCap};
use mork_capability::cnode::CapNode;
use mork_common::mork_kernel_log;
use mork_common::syscall::message_info::{InvocationLabel, MessageInfo, ResponseLabel};
use mork_common::types::{ResultWithErr, VMRights};
use mork_hal::context::HALContextTrait;
use mork_hal::mm::PageTableImpl;
use mork_mm::page_table::MutPageTableWrapper;
use mork_task::task::TaskContext;

pub fn handle(cspace: &CapNode, current: &TaskContext, dest_cap: PageTableCap, message_info: MessageInfo)
    -> ResultWithErr<MessageInfo> {
    let page_table = PageTableImpl::from_cap(&dest_cap);
    let vaddr = current.hal_context.get_mr(1);
    match InvocationLabel::from_usize(message_info.get_label()) {
        InvocationLabel::PageTableMap => {
            let page_table_cap = cspace[current.hal_context.get_mr(0)];
            if page_table_cap.get_type() != CapType::PageTable {
                return Err(MessageInfo::new_response(ResponseLabel::ErrCapType));
            }
            let mut page_table_wrapper = MutPageTableWrapper::new(page_table);
            match page_table_wrapper.map_page_table(
                vaddr,
                unsafe { page_table_cap.page_table_cap.base_ptr() << 12 } as usize
            ) {
                Ok(_) => {
                    Ok(())
                }
                Err(err) => {
                    Err(MessageInfo::new_response(err))
                }
            }

        }
        // InvocationLabel::PageTableUnmap => {}
        InvocationLabel::PageMap => {
            let vm_rights = VMRights::from_bits(current.hal_context.get_mr(2) as u8);
            if vm_rights.is_none() {
                mork_kernel_log!(warn, "Invalid vm_rights: {}", current.hal_context.get_mr(2));
                return Err(MessageInfo::new_response(ResponseLabel::InvalidParam));
            }
            let vm_rights = vm_rights.unwrap();
            let frame_cap = cspace[current.hal_context.get_mr(0)];
            if frame_cap.get_type() != CapType::Frame {
                return Err(MessageInfo::new_response(ResponseLabel::ErrCapType));
            }
            let mut page_table_wrapper = MutPageTableWrapper::new(page_table);
            match page_table_wrapper.map_frame(
                vaddr,
                unsafe { frame_cap.frame_cap.base_ptr() << 12 } as usize,
                vm_rights.contains(VMRights::X),
                vm_rights.contains(VMRights::W),
                vm_rights.contains(VMRights::R),
            ) {
                Ok(_) => { Ok(()) }
                Err(resp) => {
                    Err(MessageInfo::new_response(resp))
                }
            }
        }
        // InvocationLabel::PageUnmap => {}
        _ => {
            mork_kernel_log!(warn, "unSupported invocation label: {}", message_info.get_label());
            Err(MessageInfo::new_response(ResponseLabel::UnSupported))
        }
    }
}