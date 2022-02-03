use csv::Trim;
use proc_macro2::TokenStream;
use quote::quote;
use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{BufReader, Write},
    path::Path,
    process::Command,
};

macro_rules! line_to_vec {
    // `()` indicates that the macro takes no argument.
    ($line:expr, $expected_num_of_elements:expr) => {{
        let maybe_line_elements: Result<Vec<_>, _> = $line
            .iter()
            .filter(|s| !s.is_empty())
            .map(|s| s.parse())
            .collect();

        let line_elements = maybe_line_elements.unwrap();
        let elements_present = line_elements.len();
        let elements_expected = $expected_num_of_elements;
        assert_eq!(
            elements_present, elements_expected,
            "expected {elements_expected} elements,
                    but found {elements_present} elements"
        );

        line_elements
    }};
}

/// Parse a nnet file, and emits the equivalent token stream
fn parse_nnet<P: AsRef<Path>>(nnet_file: P, name: &str) -> TokenStream {
    // open the nnet file, create a buffered reader and feed everything to the csv crate
    let f = File::open(nnet_file).expect("file does not exits: {nnet_file}");
    let mut csv_reader = csv::ReaderBuilder::new()
        .flexible(true)
        .double_quote(false)
        .trim(Trim::All)
        .from_reader(BufReader::new(f));

    //making storage for nnet values
    let mut n_input: usize = 0;
    let mut n_mat: usize = 0;
    let mut n_neuron: usize = 0;
    let mut n_output: usize = 0;
    let mut num_layer: usize = 0;
    let mut nodes_per_layer: Vec<usize> = Vec::new();
    let mut min_input: Vec<f32> = Vec::new();
    let mut max_input: Vec<f32> = Vec::new();
    let mut mean: Vec<f32> = Vec::new();
    let mut range: Vec<f32> = Vec::new();

    //parse header
    for (line_no, line) in csv_reader.records().take(7).map(|e| e.unwrap()).enumerate() {
        // stupid humans count from one
        match line_no + 1 {
            1 => {
                let values = line_to_vec!(line, 4);

                num_layer = values[0];
                n_input = values[1];
                n_output = values[2];
                n_neuron = values[3];
                n_mat = num_layer - 2;
            }
            2 => nodes_per_layer = line_to_vec!(line, num_layer + 1),
            3 => {} // can be ignored
            4 => min_input = line_to_vec!(line, n_input),
            5 => max_input = line_to_vec!(line, n_input),
            6 => mean = line_to_vec!(line, n_input + 1),
            7 => range = line_to_vec!(line, n_input + 1),
            _ => panic!("We should have never landed here.."),
        }
    }

    //parse data
    let mut biases: Vec<Vec<f32>> = Vec::with_capacity(num_layer);
    let mut weights: Vec<Vec<Vec<f32>>> = Vec::with_capacity(num_layer);

    let mut layer = 0;

    while layer <= num_layer - 1 {
        let num_cols = nodes_per_layer[layer];
        let num_rows = nodes_per_layer[layer + 1];

        let current_weights = csv_reader
            .records() // go through the lines
            .take(num_rows) // take exactly as many as we expect lines
            .map(|maybe_record| line_to_vec!(maybe_record.unwrap(), num_cols))
            //.flatten()
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


    //nnet file has been read by now
    //splitting generated vectors into the right sizes for the struct NNet 
    // see also documentation of https://github.com/sisl/NNet
    let input_biases = biases.remove(0);
    let input_weights = weights.remove(0);

    let output_biases = biases.pop().unwrap();
    let output_weights = weights.pop().unwrap();

    let mean_output = mean.pop().unwrap();
    let range_output = range.pop().unwrap();

    let ident = quote::format_ident!("{name}");

    quote!(
        const #ident: NNet<#n_input, #n_mat, #n_neuron, #n_output> = NNet {
            input_layer: Layer {
                a: matrix![ #( #( #input_weights ),* );* ],
                biases: vector![ #( #input_biases ),* ],
            },
            hidden_layers: [
                #( Layer {
                    a: matrix![ #( #( #weights ),* );* ],
                    biases: vector![ #( #biases ),* ],
                } ),*
            ],
            output_layer: Layer {
                a: matrix![ #(
                       #( #output_weights ),*
                    );* ],
                biases: vector![ #( #output_biases ),* ],
            },
            min_input: vector![ #( #min_input ),* ],
            max_input: vector![ #( #max_input ),* ],
            mean_value: vector![ #( #mean ),* ],
            range: vector![ #( #range ),* ],
            mean_output: #mean_output,
            range_output: #range_output,
        };
    )
}


fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("nnets.rs");

    let mut f = OpenOptions::new().write(true).create(true).open(&dest_path).unwrap();

    for maybe_nnet_file in fs::read_dir("nnets")
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| e.metadata()
        .unwrap()
        .is_file() && e.file_name().to_str().unwrap().ends_with(".nnet"))
    {
        //panic!("{:?}", maybe_nnet_file.file_name());

        let path = maybe_nnet_file.path();
        let name = path.file_name().unwrap().to_str().unwrap().to_string();

        let token_tree = parse_nnet(path, &name.strip_suffix(".nnet").unwrap());


        // linebreak after `;` with nice indentation to make the matrices readable
        let indent = format!(";\n{}", " ".repeat(12));
       // fs::write(&dest_path, token_tree.to_string().replace(';', &indent)).unwrap();
        write!(&mut f, "{}", token_tree.to_string().replace(';', &indent)).unwrap();

    }

    // format the generated source code
    if let Err(e) = Command::new("rustfmt")
        .arg(dest_path.as_os_str())
        .current_dir(&out_dir)
        .status()
    {
        eprintln!("{e}")
    }

    println!("cargo:rerun-if-changed=nnets");
}
