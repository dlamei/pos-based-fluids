use pos_based_fluids::run;

fn main() {
    pollster::block_on(run());
}
