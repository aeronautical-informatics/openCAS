use csv::{ReaderBuilder, Trim};
use std::{error::Error, fs::File, io::BufReader, path::Path};

/// Parse a nnet file
fn parse_nnet<P: AsRef<Path>>(nnet_file: P) -> Result<(), Box<dyn std::error::Error>> {
    // open the nnet file, create a buffered reader and feed everything to the csv crate
    let f = File::open(nnet_file)?;
    let mut csv_reader = csv::ReaderBuilder::new()
        .flexible(true)
        .double_quote(false)
        .trim(Trim::All)
        .from_reader(BufReader::new(f));

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
    for (line_no, maybe_line) in csv_reader.records().take(7).enumerate() {
        let raw_line = maybe_line?;
        let line = raw_line.iter();

        // stupid humans count from one
        match line_no + 1 {
            1 => {
                let mut value_iter = line.map(|s| s.parse());

                num_layer = value_iter.next().ok_or("no value available")??;
                n_input = value_iter.next().ok_or("no value available")??;
                n_output = value_iter.next().ok_or("no value available")??;
                n_neuron = value_iter.next().ok_or("no value available")??;
            }
            2 => {
                let maybe_nodes_per_layer: Result<_, _> =
                    line.filter(|s| !s.is_empty()).map(|s| s.parse()).collect();
                nodes_per_layer = maybe_nodes_per_layer?;
            }
            3 => {} // can be ignored
            4 => {
                let maybe_min_input: Result<_, _> =
                    line.filter(|s| !s.is_empty()).map(|s| s.parse()).collect();
                min_input = maybe_min_input?;
            }
            5 => {
                let maybe_max_input: Result<_, _> =
                    line.filter(|s| !s.is_empty()).map(|s| s.parse()).collect();
                max_input = maybe_max_input?;
            }
            6 => {
                let maybe_mean: Result<_, _> =
                    line.filter(|s| !s.is_empty()).map(|s| s.parse()).collect();
                mean = maybe_mean?;
            }
            7 => {
                let maybe_range: Result<_, _> =
                    line.filter(|s| !s.is_empty()).map(|s| s.parse()).collect();
                range = maybe_range?;
            }
            _ => panic!(),
        }
    }

    //parse data
    let mut biases: Vec<Vec<f32>> = Vec::with_capacity(num_layer);
    let mut weights: Vec<Vec<f32>> = Vec::with_capacity(num_layer);

    let mut layer = 0;

    while layer < num_layer - 1 {
        let num_cols = nodes_per_layer[layer];
        let num_rows = nodes_per_layer[layer + 1];

        let current_weights: Vec<f32> = csv_reader
            .records() // go through the lines
            .take(num_rows) // take exactly as many as we expect lines
            .map(|maybe_record| {
                // each record (line) itself is an iterator
                let record = maybe_record
                    .unwrap();
                let result = record .iter()
                    .filter(|s| !s.is_empty())
                    .map(|s| s.parse().unwrap())
                    .collect::<Vec<_>>();
                let cols_actual = result.len();
                assert_eq!(result.len(), num_cols, "weights matrix has wrong number of columns: expected {num_cols}, found {cols_actual}") ;
                result
            })
            .flatten()
            .collect();

        let current_biases = csv_reader
            .records() // go through the lines
            .take(num_rows) // take exactly as many as we expect lines
            .map(|maybe_record| {
                // each record (line) itself is an iterator, which should be of length one
                let record = maybe_record.unwrap();
                let mut iter = record.iter().filter(|s| !s.is_empty());
                let result = iter.next().unwrap().parse().unwrap();
                let unwanted_element = iter.next();
                assert_eq!(unwanted_element, None, "biases vector is expected to have exactly one element per line, found at least another one: {unwanted_element:#?}");
                result
            })
            .collect();

        layer += 1;
        weights.push(current_weights);
        biases.push(current_biases);
    }

    panic!("{weights:#?}\n{biases:#?}",);

    // TODO construct the actual source code for the Nnet
}

#[test]
fn file_read() {
    let file_path = std::path::Path::new("assets/HCAS_rect_v6_pra0_tau00_25HU_3000.nnet");
    let result = parse_nnet(file_path);
    panic!("{result:?}");
    //let my_nnet: NNet<3, 4, 25, 5> = parse_nnet(&contents).expect("oh no");
}
