fn main() {
    cc::Build::new()
        .object("src/uarch_combined.o")
        .compile("cartesi");
}