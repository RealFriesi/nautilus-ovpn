const std = @import("std");
const Translator = @import("translate_c").Translator;

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const translate_c = b.dependency("translate_c", .{});
    const translator: Translator = .init(translate_c, .{
        .c_source_file = b.addWriteFiles().add("c.h",
            \\#include <nautilus-extension.h>
            \\#include <glib-object.h>
        ),
        .target = target,
        .optimize = optimize,
        .link_libc = true,
        .link_system_libs = &.{
            .{ .name = "libnautilus-extension-4" },
            .{ .name = "glib-2.0" },
            .{ .name = "gobject-2.0" },
        },
    });

    const lib = b.addLibrary(.{
        .name = "nautilus_ovpn",
        .linkage = .dynamic,
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/extension.zig"),
            .target = target,
            .optimize = optimize,
            .link_libc = true,
            .imports = &.{
                .{ .name = "c", .module = translator.mod },
            },
        }),
    });

    b.installArtifact(lib);
}
