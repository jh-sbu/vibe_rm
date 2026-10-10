use ruffle_macros::istr;

use crate::avm1::activation::Activation;
use crate::avm1::error::Error;
use crate::avm1::globals::as_broadcaster::BroadcasterFunctions;
use crate::avm1::property_decl::{DeclContext, StaticDeclarations};
use crate::avm1::{Object, Value};
use crate::display_object::{
    DisplayObject, EditText, TDisplayObject, TInteractiveObject, TextSelection,
};
use crate::focus_tracker::NavigationDirection;

const OBJECT_DECLS: StaticDeclarations = declare_static_properties! {
    "getBeginIndex" => method(get_begin_index; DONT_ENUM | DONT_DELETE | READ_ONLY);
    "getEndIndex" => method(get_end_index; DONT_ENUM | DONT_DELETE | READ_ONLY);
    "getCaretIndex" => method(get_caret_index; DONT_ENUM | DONT_DELETE | READ_ONLY);
    "getFocus" => method(get_focus; DONT_ENUM | DONT_DELETE | READ_ONLY);
    "setFocus" => method(set_focus; DONT_ENUM | DONT_DELETE | READ_ONLY);
    "setSelection" => method(set_selection; DONT_ENUM | DONT_DELETE | READ_ONLY);
    // Scaleform's extensions, as Skyrim's CLIK FocusHandler uses them, for one
    // controller (focus group 0, mask 1).
    "getControllerFocusGroup" => method(get_controller_focus_group; DONT_ENUM | DONT_DELETE | READ_ONLY);
    "getControllerMaskByFocusGroup" => method(get_controller_mask_by_focus_group; DONT_ENUM | DONT_DELETE | READ_ONLY);
    "getFocusBitmask" => method(get_focus_bitmask; DONT_ENUM | DONT_DELETE | READ_ONLY);
    "findFocus" => method(find_focus; DONT_ENUM | DONT_DELETE | READ_ONLY);
    "numFocusGroups" => value(1; DONT_ENUM | DONT_DELETE | READ_ONLY);
};

pub fn get_controller_focus_group<'gc>(
    _activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    Ok(0.into())
}

pub fn get_controller_mask_by_focus_group<'gc>(
    _activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    Ok(1.into())
}

/// The controllers focusing `args[0]`: 1 when it has the focus, else 0.
pub fn get_focus_bitmask<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let start_clip = activation.target_clip_or_root();
    let Some(&target) = args.first() else {
        return Ok(0.into());
    };
    let object = activation.resolve_target_display_object(start_clip, target, false)?;
    let focus = activation.context.focus_tracker.get();
    let focused = matches!((object, focus), (Some(o), Some(f)) if DisplayObject::ptr_eq(o, f.as_displayobject()));
    Ok(i32::from(focused).into())
}

/// `findFocus(nav, context, loop, startFrom, includeFocusEnabled, controller)`:
/// the focusable object in direction `nav` ("up", "down", "left", "right")
/// from `startFrom` (else the focus) inside `context`, or null.
pub fn find_focus<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let arg = |i: usize| args.get(i).copied().unwrap_or(Value::Undefined);
    let direction = match arg(0).coerce_to_string(activation)?.to_string().as_str() {
        "up" => NavigationDirection::Up,
        "down" => NavigationDirection::Down,
        "left" => NavigationDirection::Left,
        "right" => NavigationDirection::Right,
        _ => return Ok(Value::Null),
    };
    let start_clip = activation.target_clip_or_root();
    let mut object = |v: Value<'gc>| -> Result<Option<DisplayObject<'gc>>, Error<'gc>> {
        if matches!(v, Value::Undefined | Value::Null) {
            return Ok(None);
        }
        activation.resolve_target_display_object(start_clip, v, false)
    };
    let within = object(arg(1))?;
    let from = object(arg(3))?.and_then(|o| o.as_interactive());
    let tracker = activation.context.focus_tracker;
    Ok(tracker
        .find(activation.context, direction, from, within)
        .and_then(|o| o.as_displayobject().object1())
        .map_or(Value::Null, Value::from))
}

pub fn create<'gc>(
    context: &mut DeclContext<'_, 'gc>,
    broadcaster_fns: BroadcasterFunctions<'gc>,
    array_proto: Object<'gc>,
) -> Object<'gc> {
    let selection = Object::new(context.strings, Some(context.object_proto));
    broadcaster_fns.initialize(context.strings, selection, array_proto);
    context.define_properties_on(selection, OBJECT_DECLS(context));
    selection
}

pub fn get_begin_index<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    if let Some(selection) = activation
        .context
        .focus_tracker
        .get_as_edit_text()
        .and_then(EditText::selection)
    {
        Ok(Value::from_usize_lossy(selection.start()))
    } else {
        Ok((-1).into())
    }
}

pub fn get_end_index<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    if let Some(selection) = activation
        .context
        .focus_tracker
        .get_as_edit_text()
        .and_then(EditText::selection)
    {
        Ok(Value::from_usize_lossy(selection.end()))
    } else {
        Ok((-1).into())
    }
}

pub fn get_caret_index<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    if let Some(selection) = activation
        .context
        .focus_tracker
        .get_as_edit_text()
        .and_then(EditText::selection)
    {
        Ok(Value::from_usize_lossy(selection.to()))
    } else {
        Ok((-1).into())
    }
}

pub fn set_selection<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    if args.is_empty() {
        return Ok(Value::Undefined);
    }

    if let Some(edit_box) = activation.context.focus_tracker.get_as_edit_text() {
        let start = args
            .get(0)
            .map(|v| v.coerce_to_i32(activation))
            .transpose()?
            .unwrap_or(0)
            .max(0);
        let end = args
            .get(1)
            .map(|v| v.coerce_to_i32(activation))
            .transpose()?
            .unwrap_or(i32::MAX)
            .max(0);
        let selection = TextSelection::for_range(start as usize, end as usize);
        edit_box.set_selection(Some(selection));
    }
    Ok(Value::Undefined)
}

pub fn get_focus<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let focus = activation.context.focus_tracker.get();
    Ok(match focus {
        Some(focus) => focus
            .as_displayobject()
            .object1_or_undef()
            .coerce_to_string(activation)
            .unwrap_or_else(|_| istr!(""))
            .into(),
        None => Value::Null,
    })
}

pub fn set_focus<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let tracker = activation.context.focus_tracker;
    match args.get(0) {
        None => Ok(false.into()),
        Some(Value::Undefined | Value::Null) => {
            tracker.set(None, activation.context);
            Ok(true.into())
        }
        Some(focus) => {
            let start_clip = activation.target_clip_or_root();
            let object = activation.resolve_target_display_object(start_clip, *focus, false)?;
            if let Some(object) = object
                && let Some(object) = object.as_interactive()
                && object.is_focusable(activation.context)
            {
                tracker.set(Some(object), activation.context);
                return Ok(true.into());
            }
            Ok(false.into())
        }
    }
}
