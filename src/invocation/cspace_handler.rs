use alloc::alloc::{alloc_zeroed, dealloc};
use core::alloc::Layout;
use mork_capability::cap::{CNodeCap, CapType, FrameCap, NotificationCap, PageTableCap, ThreadCap};
use mork_capability::cnode::CapNode;
use mork_capability::free_callback::CallbackHandler;
use mork_common::constants::{CNodeSlot, ObjectType, MAX_CNODE_SIZE, MAX_THREAD_PIRO};
use mork_common::mork_kernel_log;
use mork_common::syscall::message_info::{InvocationLabel, MessageInfo, ResponseLabel};
use mork_hal::config::{PAGE_SIZE_2M, PAGE_SIZE_NORMAL};
use mork_hal::context::HALContextTrait;
use mork_mm::page_table::PageTable;
use mork_task::task::TaskContext;
use mork_ipc::notification::Notification;

pub fn handle(current: &mut TaskContext, dest_cap: ThreadCap, message_info: MessageInfo) -> Result<usize, MessageInfo> {
    let task = TaskContext::from_cap(&dest_cap);
    if task.cspace.is_none() {
        mork_kernel_log!(error, "there is no cspace");
        return Err(MessageInfo::new_response(ResponseLabel::NotEnoughSpace));
    }

    let cspace = task.cspace.as_mut().unwrap();
    match InvocationLabel::from_usize(message_info.get_label()) {
        InvocationLabel::CNodeAlloc => {
            let mut handler = AllocHandler { cspace };
            let obj_type = ObjectType::from_usize(current.hal_context.get_mr(0));
            handler.handle(obj_type)
        }

        InvocationLabel::CNodeDelete => {
            let object_idx = current.hal_context.get_mr(0);
            cspace.free_slot(object_idx);
            Ok(object_idx)
        }
        InvocationLabel::CNodeCopy => {
            let src_cap = cspace[current.hal_context.get_mr(0)];
            let input_cap = cspace[current.hal_context.get_mr(1)];
            let dest_slot = current.hal_context.get_mr(2);
            if input_cap.get_type() != CapType::Thread {
                mork_kernel_log!(warn, "except thread cap, found: {:?}", input_cap.get_type());
                return Err(MessageInfo::new_response(ResponseLabel::ErrCapType));
            }
            let dest_task_cap = unsafe { input_cap.thread_cap };
            let dest_task = TaskContext::from_cap(&dest_task_cap);
            if let Some(dest_cspace) = dest_task.cspace.as_mut() {
                if dest_cspace.is_used(dest_slot) {
                    if let Some(slot) = dest_cspace.alloc_free() {
                        dest_cspace[slot] = src_cap.derive();
                        Ok(slot)
                    } else {
                        mork_kernel_log!(warn, "dest cspace not found");
                        Err(MessageInfo::new_response(ResponseLabel::NotEnoughSpace))
                    }
                } else {
                    dest_cspace[dest_slot] = src_cap.derive();
                    Ok(dest_slot)
                }
            } else {
                mork_kernel_log!(warn, "dest cspace not found");
                Err(MessageInfo::new_response(ResponseLabel::NotEnoughSpace))
            }
        }
        _ => {
            mork_kernel_log!(warn, "unSupported invocation label: {}", message_info.get_label());
            Err(MessageInfo::new_response(ResponseLabel::UnSupported))
        }
    }
}

struct AllocHandler<'a> {
    pub cspace: &'a mut CapNode
}

impl AllocHandler<'_> {
    pub fn handle(&mut self, object_type: ObjectType) -> Result<usize, MessageInfo> {
        if let Some(slot) = self.cspace.alloc_free() {
            let (size, align) = Self::get_layout(&object_type);
            let layout = Layout::from_size_align(size, align).unwrap();
            let object_ptr = unsafe { alloc_zeroed(layout) };
            if object_ptr.is_null() {
                mork_kernel_log!(warn, "Alloc memory failed");
                Err(MessageInfo::new_response(ResponseLabel::NotEnoughSpace))
            } else {
                match object_type {
                    ObjectType::Frame4K => {
                        let cap = FrameCap::new(object_ptr as usize, 3);
                        self.cspace[slot] = cap.into_cap();
                        Ok(slot)
                    }
                    ObjectType::Frame2M => {
                        let cap = FrameCap::new(object_ptr as usize, 2);
                        self.cspace[slot] = cap.into_cap();
                        Ok(slot)
                    }
                    ObjectType::PageTable => {
                        let cap = PageTableCap::new(object_ptr as usize);
                        self.cspace[slot] = cap.into_cap();
                        Ok(slot)
                    }
                    ObjectType::Thread => {
                        let cap = ThreadCap::new(object_ptr as usize);
                        let task = TaskContext::from_cap(&cap);
                        *task = TaskContext::new_user_thread();
                        task.init_cspace();
                        task.prio = MAX_THREAD_PIRO - 1;
                        self.cspace[slot] = cap.into_cap();
                        Ok(slot)
                    }
                    ObjectType::Notification => {
                        let cap = NotificationCap::new(object_ptr as usize);
                        let notification = Notification::from_cap(&cap);
                        *notification = Notification::new();
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

    fn get_layout(object_type: &ObjectType) -> (usize, usize) {
        match object_type {
            ObjectType::CNode => (size_of::<CapNode>(), PAGE_SIZE_NORMAL),
            ObjectType::Thread => (size_of::<TaskContext>(), PAGE_SIZE_NORMAL),
            ObjectType::PageTable => (size_of::<PageTable>(), PAGE_SIZE_NORMAL),
            ObjectType::Frame4K => (PAGE_SIZE_NORMAL, PAGE_SIZE_NORMAL),
            ObjectType::Frame2M => (PAGE_SIZE_2M, PAGE_SIZE_2M),
            ObjectType::Notification => (size_of::<Notification>(), PAGE_SIZE_NORMAL),
            _ => {
                panic!("unsupported object type")
            }
        }
    }
}

pub struct DeallocHandler;

impl CallbackHandler for DeallocHandler {
    fn free_cnode(&self, _cap: CNodeCap) {
        panic!("should not be invoked!")
    }

    fn free_frame(&self, cap: FrameCap) {
        let base_ptr = (cap.base_ptr() << 12) as usize;
        let (size, align) = match cap.level() {
            3 => {
                (PAGE_SIZE_NORMAL, PAGE_SIZE_NORMAL)
            }
            2 => {
                (PAGE_SIZE_2M, PAGE_SIZE_2M)
            }
            _ => {
                panic!("unsupported object type")
            }
        };
        let layout = Layout::from_size_align(size, align).unwrap();
        unsafe {
            dealloc(base_ptr as *mut u8, layout);
        }
    }

    fn free_page_table(&self, cap: PageTableCap) {
        let base_ptr = (cap.base_ptr() << 12) as usize;
        let layout = Layout::from_size_align(size_of::<PageTable>(), PAGE_SIZE_NORMAL).unwrap();
        unsafe {
            dealloc(base_ptr as *mut u8, layout);
        }
    }

    fn free_task(&self, cap: ThreadCap) {
        let task = TaskContext::from_cap(&cap);
        if let Some(cspace) = task.cspace.take() {
            for i in CNodeSlot::CapInitVSpace as usize..MAX_CNODE_SIZE {
                if cspace.empty() {
                    break;
                }
                if cspace.is_used(i) {
                    cspace[i].free();
                }
            }
        }
        let base_ptr = task.get_ptr();
        let layout = Layout::from_size_align(size_of::<TaskContext>(), PAGE_SIZE_NORMAL).unwrap();
        unsafe {
            dealloc(base_ptr as *mut u8, layout);
        }
    }

    fn free_notification(&self, cap: NotificationCap) {
        let base_ptr = (cap.base_ptr() << 12) as usize;
        let layout = Layout::from_size_align(size_of::<Notification>(), PAGE_SIZE_NORMAL).unwrap();
        unsafe {
            dealloc(base_ptr as *mut u8, layout);
        }
    }
}