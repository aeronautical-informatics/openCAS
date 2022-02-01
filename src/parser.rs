use crate::inference::NNet;

/// Parse a nnet file
fn parse_nnet(nnet: &str) -> Result<(), Box<dyn std::error::Error>> {
    //let mut my_nnet = NNet::default();
    let mut n_input: usize = 0;
    let mut n_mat: usize = 0;
    let mut n_neuron: usize = 0;
    let mut n_output: usize = 0;
    let mut num_layer: usize = 0;
    let mut nodes_per_layer: Vec<usize> = Vec::new();
    let mut min_input: Vec<f32> = Vec::new();
    let mut max_input: Vec<f32> = Vec::new();
    let mut max_input: Vec<f32> = Vec::new();
    let mut mean: Vec<f32> = Vec::new();
    let mut range: Vec<f32> = Vec::new();

    //parse header
    for (line_no, line) in nnet.lines().take(8).enumerate() {
        match line_no + 1 {
            // stupid humans count from one
            1 => {} // header text
            2 => {
                let mut value_iter = line.split(",").map(|s| s.parse());
                num_layer = value_iter.next().ok_or("no value available")??;
                n_input = value_iter.next().ok_or("no value available")??;
                n_output = value_iter.next().ok_or("no value available")??;
                n_neuron = value_iter.next().ok_or("no value available")??;
            }
            3 => {
                let maybe_nodes_per_layer: Result<_, _> =
                    line.split(',').map(|s| s.parse()).collect();
                nodes_per_layer = maybe_nodes_per_layer?;
            }
            4 => {} // can be ignored
            5 => {
                let maybe_min_input: Result<_, _> = line.split(',').map(|s| s.parse()).collect();
                min_input = maybe_min_input?;
            }
            6 => {
                let maybe_max_input: Result<_, _> = line.split(',').map(|s| s.parse()).collect();
                max_input = maybe_max_input?;
            }
            7 => {
                let maybe_mean: Result<_, _> = line.split(',').map(|s| s.parse()).collect();
                mean = maybe_mean?;
            }
            8 => {
                let maybe_range: Result<_, _> = line.split(',').map(|s| s.parse()).collect();
                range = maybe_range?;
            }
            _ => panic!(),
        }
    }

    //parse data
    let mut biases: Vec<Vec<f32>> = Vec::new();
    let mut weights: Vec<Vec<f32>> = Vec::new();
    let mut lines_to_skip = 8;

    //iterate through nodes_per_layer
    for (i, cols) in nodes_per_layer
        .iter()
        .enumerate()
        .take(nodes_per_layer.len() - 1)
    {
        let num_lines = nodes_per_layer[i + 1];
        for line in nnet.lines().skip(lines_to_skip).take(num_lines) {
            let maybe_weights: Result<_, _> = line.split(',').map(|s| s.parse()).collect();
            weights.push(maybe_weights?);
        }
        lines_to_skip += num_lines;

        let new_biases: Result<_, _> = nnet
            .lines()
            .skip(lines_to_skip)
            .take(num_lines)
            .map(|s| s.parse())
            .collect();
        biases.push(new_biases?);
        lines_to_skip += num_lines;
    }

    let my_nnet = true;
    panic!("{:?}", my_nnet);
    //Ok(my_nnet)
    //todo!();
}

#[test]
fn file_read() {
    let contents = std::fs::read_to_string("assets/HCAS_rect_v6_pra0_tau00_25HU_3000.nnet")
        .expect("Something went wrong reading the file");
    //let my_nnet: NNet<3, 4, 25, 5> = parse_nnet(&contents).expect("oh no");
}
