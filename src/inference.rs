use nalgebra::base::{SMatrix, SVector};

pub type Matrix<const ROWS: usize, const COLS: usize> = SMatrix<f32, ROWS, COLS>;
pub type Vector<const ROWS: usize> = SVector<f32, ROWS>;

/// A simple Neuronal Network
///
/// + `N_INPUT` is the count of input variables
/// + `N_MAT` is the number of matrices transforming betweent the hidden layers, so if there are
///   `n` hidden layers `N_MAT == n - 1`
/// + `N_NEURON` is the count of neurons per layer
/// + `N_OUTPUT` is the number of output variables
pub struct NNet<
    const N_INPUT: usize,
    const N_MAT: usize,
    const N_NEURON: usize,
    const N_OUTPUT: usize,
> {
    pub input_layer: Layer<N_INPUT, N_NEURON>,
    pub hidden_layers: [Layer<N_NEURON, N_NEURON>; N_MAT],
    pub output_layer: Layer<N_NEURON, N_OUTPUT>,
}

/// One layer of a neuronal network, consisting of a matrix of weights and a vector of biases.
///
/// + The matrix is of the dimension `OUTPUT_NEURONS` rows x `INPUT_NEURONS` columns
/// + The vector is of the dimension `OUTPUT_NEURONS`
pub struct Layer<const INPUT_NEURONS: usize, const OUTPUT_NEURONS: usize> {
    pub a: Matrix<OUTPUT_NEURONS, INPUT_NEURONS>,
    pub biases: Vector<OUTPUT_NEURONS>,
}

impl<const N_INPUT: usize, const N_MAT: usize, const N_NEURON: usize, const N_OUTPUT: usize>
    NNet<N_INPUT, N_MAT, N_NEURON, N_OUTPUT>
{
    /// Evaluates a neuronal network with specific inputs
    pub fn eval(&self, inputs: &Vector<N_INPUT>) -> Vector<N_OUTPUT> {
        // TODO normalize inputs

        // TODO check the actual core of the algorithm, is it correct?
        let mut accumulator = self.input_layer.a * inputs + self.input_layer.biases;

        for layer in &self.hidden_layers {
            accumulator = layer.a * accumulator + layer.biases;
        }

        let output = self.output_layer.a * accumulator + self.output_layer.biases;

        // TODO normalize outputs

        output
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Parse a nnet file
    fn parse_nnnet<
        const N_INPUT: usize,
        const N_MAT: usize,
        const N_NEURON: usize,
        const N_OUTPUT: usize,
    >(
        nnet: &str,
    ) -> Result<NNet<N_INPUT, N_MAT, N_NEURON, N_OUTPUT>, Box<dyn std::error::Error>> {
        for (line_no, line) in nnet.lines().enumerate() {
            match line_no + 1 {
                // stupid humans count from one
                1 => {} // header text
                2 => {
                    let mut value_iter = line.split(",").map(|s| s.parse());
                    let number_of_layers: usize =
                        value_iter.next().ok_or("no value available")??;
                    let number_of_inputs: usize =
                        value_iter.next().ok_or("no value available")??;
                    let number_of_outputs: usize =
                        value_iter.next().ok_or("no value available")??;
                    let maximum_layer_size: usize =
                        value_iter.next().ok_or("no value available")??;

                    assert_eq!(number_of_inputs, N_INPUT);
                    assert_eq!(number_of_outputs, N_OUTPUT);
                }
                4 => {} // can be ignored

                _ => todo!(),
            }
        }

        todo!()
    }

    #[test]
    fn first_test() {
        let my_vec: Vector<3> = Vector::from_column_slice(&[1.0, 2.0, 3.0]);

        let my_mat: Matrix<2, 3> = Matrix::from_row_slice(&[
            1.0, 2.0, // first row
            3.0, 4.0, // second row
            1.0, 8.0, // third row
        ]);

        panic!("Oh no! Just kidding, we hijack the fact that panic prints are outputted on test runs to print from a test:\n{:?}", my_mat * my_vec);
    }
}
