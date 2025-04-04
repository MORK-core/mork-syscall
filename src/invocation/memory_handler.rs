use mork_capability::cap::{Cap, CapType, PageTableCap};
use mork_capability::cnode::CapNode;
use mork_common::mork_kernel_log;
use mork_common::syscall::message_info::{InvocationLabel, MessageInfo, ResponseLabel};
use mork_common::types::{ResultWithErr, VMRights};
use mork_hal::context::HALContextTrait;
use mork_mm::page_table::{MutPageTableWrapper, PageTable};
use mork_task::task::TaskContext;

pub fn handle(cspace: &mut CapNode, current: &TaskContext, dest_cap: PageTableCap, message_info: MessageInfo)
              -> ResultWithErr<MessageInfo> {
    let page_table = PageTable::from_cap(&dest_cap);
    let vaddr = current.hal_context.get_mr(1);
    match InvocationLabel::from_usize(message_info.get_label()) {
        InvocationLabel::PageTableMap => {
            page_table_map(cspace, page_table, current, vaddr)
        }
        InvocationLabel::PageTableUnmap => {
            page_table_unmap(cspace, page_table, current)
        }
        InvocationLabel::PageMap => {
            page_map(cspace, page_table, current, vaddr)
        }
        InvocationLabel::PageUnmap => {
            page_unmap(cspace, page_table, current)
        }
        _ => {
            mork_kernel_log!(warn, "unSupported invocation label: {}", message_info.get_label());
            Err(MessageInfo::new_response(ResponseLabel::UnSupported))
        }
    }
}

fn page_table_map(cspace: &mut CapNode, vspace: &mut PageTable, current: &TaskContext, vaddr: usize)
                  -> ResultWithErr<MessageInfo> {
    let page_table_cap = cspace[current.hal_context.get_mr(0)];
    if page_table_cap.get_type() != CapType::PageTable {
        return Err(MessageInfo::new_response(ResponseLabel::ErrCapType));
    }
    let mut page_table_cap = unsafe { page_table_cap.page_table_cap };
    if page_table_cap.is_mapped() != 0 {
        return Err(MessageInfo::new_response(ResponseLabel::InvalidParam));
    }
    let mut page_table_wrapper = MutPageTableWrapper::new(vspace);
    match page_table_wrapper.map_page_table(
        vaddr,
        (page_table_cap.base_ptr() << 12) as usize
    ) {
        Ok(level) => {
            page_table_cap.set_mapped(1);
            page_table_cap.set_mapped_addr(vaddr as u128 >> 12);
            page_table_cap.set_level(level as u128);
            cspace[current.hal_context.get_mr(0)] = Cap { page_table_cap };
            Ok(())
        }
        Err(err) => {
            Err(MessageInfo::new_response(err))
        }
    }
}

fn page_table_unmap(cspace: &mut CapNode, vspace: &mut PageTable, current: &TaskContext)
    -> ResultWithErr<MessageInfo> {
    let page_table_cap = cspace[current.hal_context.get_mr(0)];
    if page_table_cap.get_type() != CapType::PageTable {
        return Err(MessageInfo::new_response(ResponseLabel::ErrCapType));
    }
    let mut page_table_cap = unsafe { page_table_cap.page_table_cap };
    let level = page_table_cap.mapped_level() as usize;
    if page_table_cap.is_mapped() == 0 || level == 0 {
        return Err(MessageInfo::new_response(ResponseLabel::InvalidParam));
    }
    let vaddr = (page_table_cap.mapped_addr() << 12) as usize;
    let paddr = (page_table_cap.base_ptr() << 12) as usize;
    let mut page_table_wrapper = MutPageTableWrapper::new(vspace);
    match page_table_wrapper.unmap_page_table(vaddr, paddr, level) {
        Ok(_) => {
            page_table_cap.set_mapped(0);
            page_table_cap.set_mapped_addr(0);
            page_table_cap.set_level(0);
            cspace[current.hal_context.get_mr(0)] = Cap { page_table_cap };
            Ok(())
        }
        Err(resp) => {
            Err(MessageInfo::new_response(resp))
        }
    }
}

fn page_map(cspace: &mut CapNode, vspace: &mut PageTable, current: &TaskContext, vaddr: usize)
            -> ResultWithErr<MessageInfo> {
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
    let mut frame_cap = unsafe { frame_cap.frame_cap };
    if frame_cap.is_mapped() != 0 {
        return Err(MessageInfo::new_response(ResponseLabel::InvalidParam));
    }
    let mut page_table_wrapper = MutPageTableWrapper::new(vspace);
    match page_table_wrapper.map_frame(
        vaddr,
        (frame_cap.base_ptr() << 12) as usize,
        vm_rights.contains(VMRights::X),
        vm_rights.contains(VMRights::W),
        vm_rights.contains(VMRights::R),
    ) {
        Ok(_) => {
            frame_cap.set_mapped(1);
            frame_cap.set_mapped_addr(vaddr as u128 >> 12);
            cspace[current.hal_context.get_mr(0)] = Cap { frame_cap };
            Ok(())
        }
        Err(resp) => {
            Err(MessageInfo::new_response(resp))
        }
    }
}

fn page_unmap(cspace: &mut CapNode, vspace: &mut PageTable, current: &TaskContext)
              -> ResultWithErr<MessageInfo> {
    let frame_cap = cspace[current.hal_context.get_mr(0)];
    if frame_cap.get_type() != CapType::Frame {
        return Err(MessageInfo::new_response(ResponseLabel::ErrCapType));
    }
    let mut frame_cap = unsafe { frame_cap.frame_cap };
    if frame_cap.is_mapped() == 0 {
        return Err(MessageInfo::new_response(ResponseLabel::InvalidParam));
    }
    let mapped_vaddr = (frame_cap.mapped_addr() << 12) as usize;
    let mut page_table_wrapper = MutPageTableWrapper::new(vspace);
    match page_table_wrapper.unmap_frame(mapped_vaddr) {
        Ok(_) => {
            frame_cap.set_mapped_addr(0);
            frame_cap.set_mapped(0);
            cspace[current.hal_context.get_mr(0)] = Cap { frame_cap };
            Ok(())
        }
        Err(resp) => {
            Err(MessageInfo::new_response(resp))
        }
    }
}