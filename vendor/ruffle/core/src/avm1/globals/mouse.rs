use crate::avm1::activation::Activation;
use crate::avm1::error::Error;
use crate::avm1::globals::as_broadcaster::BroadcasterFunctions;
use crate::avm1::property_decl::{DeclContext, StaticDeclarations};
use crate::avm1::{Object, Value};
use crate::avm1_stub;
use crate::display_object::{TDisplayObject, TInteractiveObject};

const OBJECT_DECLS: StaticDeclarations = declare_static_properties! {
    "show" => method(show_mouse; DONT_DELETE | DONT_ENUM | READ_ONLY);
    "hide" => method(hide_mouse; DONT_DELETE | DONT_ENUM | READ_ONLY);
    "setTrailer" => method(set_trailer; DONT_DELETE | DONT_ENUM | READ_ONLY);
    "setTrailerPosition" => method(set_trailer_position; DONT_DELETE | DONT_ENUM | READ_ONLY);
    "setTrailerMode" => method(set_trailer_mode; DONT_DELETE | DONT_ENUM | READ_ONLY);
    // Scaleform's extension (Skyrim's item menus test what the cursor is over).
    "getTopMostEntity" => method(get_top_most_entity; DONT_DELETE | DONT_ENUM | READ_ONLY);
};

pub fn create<'gc>(
    context: &mut DeclContext<'_, 'gc>,
    broadcaster_functions: BroadcasterFunctions<'gc>,
    array_proto: Object<'gc>,
) -> Object<'gc> {
    let mouse = Object::new(context.strings, Some(context.object_proto));
    broadcaster_functions.initialize(context.strings, mouse, array_proto);
    context.define_properties_on(mouse, OBJECT_DECLS(context));
    mouse
}

pub fn show_mouse<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let was_visible = activation.context.ui.mouse_visible();
    activation.context.ui.set_mouse_visible(true);
    Ok(if was_visible { 0 } else { 1 }.into())
}

pub fn hide_mouse<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let was_visible = activation.context.ui.mouse_visible();
    activation.context.ui.set_mouse_visible(false);
    Ok(if was_visible { 0 } else { 1 }.into())
}

fn set_trailer<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    avm1_stub!(activation, "Mouse", "setTrailer");
    Ok(Value::Undefined)
}

fn set_trailer_position<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    avm1_stub!(activation, "Mouse", "setTrailerPosition");
    Ok(Value::Undefined)
}

fn set_trailer_mode<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    _args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    avm1_stub!(activation, "Mouse", "setTrailerMode");
    Ok(Value::Undefined)
}

/// Scaleform's `Mouse.getTopMostEntity([testAll])`: the topmost movie clip,
/// button or text field under the cursor (with `testAll` false, only those
/// with mouse handlers), `undefined` over none. Scaleform's other forms (a
/// mouse index, or a point) aren't done: the cursor's position is used.
fn get_top_most_entity<'gc>(
    activation: &mut Activation<'_, 'gc>,
    _this: Object<'gc>,
    args: &[Value<'gc>],
) -> Result<Value<'gc>, Error<'gc>> {
    let test_all = match args.first() {
        Some(v @ Value::Bool(_)) => v.as_bool(activation.swf_version()),
        _ => true,
    };
    let picked = crate::player::run_mouse_pick(activation.context, !test_all);
    Ok(picked.map_or(Value::Undefined, |o| o.as_displayobject().object1_or_undef()))
}
