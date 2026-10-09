//! By convention, root.zig is the root source file when making a package.
const std = @import("std");
const c = @import("c");

const OvpnExtension = extern struct {
    parent_instance: c.GObject,
};

const OvpnExtensionClass = extern struct {
    parent_class: c.GObjectClass,
};

var ovpn_extension_type: c.GType = 0;
var registered_types = [_]c.GType{0};

fn selectedFile(files: ?*c.GList) ?*c.NautilusFileInfo {
    const current = files orelse return null;
    if (current.next != null) return null; // Nur bei genau einer ausgewählten Datei

    const data = current.data orelse return null;
    return @ptrCast(@alignCast(data));
}

fn isOvpnFile(info: *c.NautilusFileInfo) bool {
    const name = c.nautilus_file_info_get_name(info) orelse return false;
    defer c.g_free(name);

    return std.mem.endsWith(u8, std.mem.span(name), ".ovpn");
}

fn getFileItems(_: ?*c.NautilusMenuProvider, files: ?*c.GList) callconv(.c) ?*c.GList {
    const info = selectedFile(files) orelse return null;
    if (!isOvpnFile(info)) return null;

    const item = c.nautilus_menu_item_new(
        "OvpnExtension::connect",
        "Verbinden",
        "Mit VPN verbinden",
        "network-vpn-symbolic",
    );

    return c.g_list_append(null, item);
}

fn menuProviderInterfaceInit(ptr: ?*anyopaque, _: ?*anyopaque) callconv(.c) void {
    const iface: *c.NautilusMenuProviderInterface = @ptrCast(@alignCast(ptr));
    iface.get_file_items = &getFileItems;
}

fn registerExtension(module: *c.GTypeModule) void {
    var type_info = std.mem.zeroes(c.GTypeInfo);
    type_info.class_size = @sizeOf(OvpnExtensionClass);
    type_info.instance_size = @sizeOf(OvpnExtension);

    ovpn_extension_type = c.g_type_module_register_type(
        module,
        c.g_object_get_type(),
        "OvpnExtension",
        &type_info,
        0,
    );

    var iface_info = std.mem.zeroes(c.GInterfaceInfo);
    iface_info.interface_init = &menuProviderInterfaceInit;

    c.g_type_module_add_interface(
        module,
        ovpn_extension_type,
        c.nautilus_menu_provider_get_type(),
        &iface_info,
    );
}

// Exports
pub export fn nautilus_module_initialize(module: *c.GTypeModule) callconv(.c) void {
    registerExtension(module);
}

pub export fn nautilus_module_shutdown() callconv(.c) void {}

pub export fn nautilus_module_list_types(types: [*c][*c]c.GType, num_types: *c.gint) callconv(.c) void {
    registered_types[0] = ovpn_extension_type;
    types.* = &registered_types;
    num_types.* = 1;
}
