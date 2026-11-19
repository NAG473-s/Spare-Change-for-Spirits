fn main() {
    println!("cargo:rerun-if-changed=windows.rc");
    println!("cargo:rerun-if-changed=art/icons/spare-change-for-spirits.ico");
    embed_resource::compile_for(
        "windows.rc",
        ["spare_change_for_spirits"],
        embed_resource::NONE,
    )
    .manifest_required()
    .expect("could not embed the Windows application icon");
}
