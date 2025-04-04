use alloc::alloc::alloc_zeroed;
use core::alloc::Layout;
use mork_capability::cap::{FrameCap, PageTableCap};
use mork_capability::cnode::CapNode;
use mork_common::constants::ObjectType;
use mork_common::mork_kernel_log;
use mork_common::syscall::message_info::{MessageInfo, ResponseLabel};
use mork_hal::config::NORMAL_PAGE_SIZE;
use mork_mm::page_table::PageTable;
use mork_task::task::TaskContext;

pub struct AllocHandler<'a> {
    pub cspace: &'a mut CapNode
}

impl AllocHandler<'_> {
    pub fn handle(&mut self, object_type: ObjectType) -> Result<usize, MessageInfo> {
        const OBJECT_ALIGN: usize = 4096;
        if let Some(slot) = self.cspace.alloc_free() {
            let size = Self::get_object_size(&object_type);
            let layout = Layout::from_size_align(size, OBJECT_ALIGN).unwrap();
            let object_ptr = unsafe { alloc_zeroed(layout) };
            if object_ptr.is_null() {
                mork_kernel_log!(warn, "Alloc memory failed");
                Err(MessageInfo::new_response(ResponseLabel::NotEnoughSpace))
            } else {
                match object_type {
                    ObjectType::Frame => {
                        let cap = FrameCap::new(object_ptr as usize);
                        self.cspace[slot] = cap.into_cap();
                        Ok(slot)
                    }
                    ObjectType::PageTable => {
                        let cap = PageTableCap::new(object_ptr as usize);
                        self.cspace[slot] = cap.into_cap();
                        Ok(slot)
                    }
                    _ => {
                        todo!("not supported")
                    }
                }
            }
        } else {
            mork_kernel_log!(warn, "Alloc free slot failed");
            Err(MessageInfo::new_response(ResponseLabel::NotEnoughSpace))
        }
    }

    fn get_object_size(object_type: &ObjectType) -> usize {
        match object_type {
            ObjectType::CNode => size_of::<CapNode>(),
            ObjectType::Thread => size_of::<TaskContext>(),
            ObjectType::PageTable => size_of::<PageTable>(),
            ObjectType::Frame => NORMAL_PAGE_SIZE,
            _ => {
                panic!("unsupported object type")
            }
        }
    }
}