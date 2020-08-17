use super::objtype::PyClassRef;
use crate::common::hash::PyHash;
use crate::function::{OptionalArg, PyFuncArgs};
use crate::pyobject::{
    IdProtocol, PyClassImpl, PyContext, PyObjectRef, PyRef, PyResult, PyValue, TypeProtocol,
};
use crate::pyobjectrc::{PyObjectRc, PyObjectWeak};
use crate::slots::{Hashable, SlotCall};
use crate::vm::VirtualMachine;

use crossbeam_utils::atomic::AtomicCell;

#[pyclass(module = false, name = "weakref")]
#[derive(Debug)]
pub struct PyWeak {
    referent: PyObjectWeak,
    hash: AtomicCell<Option<PyHash>>,
}

impl PyWeak {
    pub fn downgrade(obj: &PyObjectRef) -> PyWeak {
        PyWeak {
            referent: PyObjectRc::downgrade(obj),
            hash: AtomicCell::new(None),
        }
    }

    pub fn upgrade(&self) -> Option<PyObjectRef> {
        PyObjectRc::upgrade_weak(&self.referent)
    }
}

impl PyValue for PyWeak {
    fn class(vm: &VirtualMachine) -> PyClassRef {
        vm.ctx.types.weakref_type.clone()
    }
}

pub type PyWeakRef = PyRef<PyWeak>;

impl SlotCall for PyWeak {
    fn call(&self, args: PyFuncArgs, vm: &VirtualMachine) -> PyResult {
        args.bind::<()>(vm)?;
        Ok(self.upgrade().unwrap_or_else(|| vm.get_none()))
    }
}

#[pyimpl(with(SlotCall, Hashable), flags(BASETYPE))]
impl PyWeak {
    // TODO callbacks
    #[pyslot]
    fn tp_new(
        cls: PyClassRef,
        referent: PyObjectRef,
        _callback: OptionalArg<PyObjectRef>,
        vm: &VirtualMachine,
    ) -> PyResult<PyRef<Self>> {
        PyWeak::downgrade(&referent).into_ref_with_type(vm, cls)
    }

    #[pymethod(magic)]
    fn eq(&self, other: PyObjectRef, vm: &VirtualMachine) -> PyResult {
        if let Some(other) = other.payload_if_subclass::<Self>(vm) {
            self.upgrade()
                .and_then(|s| other.upgrade().map(|o| (s, o)))
                .map_or(Ok(false), |(a, b)| vm.bool_eq(a, b))
                .map(|b| vm.ctx.new_bool(b))
        } else {
            Ok(vm.ctx.not_implemented())
        }
    }

    #[pymethod(magic)]
    fn repr(zelf: PyRef<Self>) -> String {
        let id = zelf.get_id();
        if let Some(o) = zelf.upgrade() {
            format!(
                "<weakref at {}; to '{}' at {}>",
                id,
                o.lease_class().name,
                o.get_id(),
            )
        } else {
            format!("<weakref at {}; dead>", id)
        }
    }
}

impl Hashable for PyWeak {
    fn hash(zelf: PyRef<Self>, vm: &VirtualMachine) -> PyResult<PyHash> {
        match zelf.hash.load() {
            Some(hash) => Ok(hash),
            None => {
                let obj = zelf
                    .upgrade()
                    .ok_or_else(|| vm.new_type_error("weak object has gone away".to_owned()))?;
                let hash = vm._hash(&obj)?;
                zelf.hash.store(Some(hash));
                Ok(hash)
            }
        }
    }
}

pub fn init(context: &PyContext) {
    PyWeak::extend_class(context, &context.types.weakref_type);
}
