use nalgebra::base::{SMatrix, SVector};

pub type Matrix<const ROWS: usize, const COLS: usize> = SMatrix<f32, ROWS, COLS>;
pub type Vector<const ROWS: usize> = SVector<f32, ROWS>;

/// A simple Neuronal Network
///
/// + `N_INPUT` is the count of input variables
/// + `N_MAT` is the number of matrices transforming between the hidden layers, so if there are
///   `n` hidden layers `N_MAT == n - 1`
/// + `N_NEURON` is the count of neurons per layer
/// + `N_OUTPUT` is the number of output variables
///
/// The struct fields contain all information about the network.
///
/// + `input_layer` holds the weight matrix and bias vector for calculating the transitions
///    from the input neurons the the neurons of the first hidden layer.
/// + `hidden_layers` hold all `N_MAT` matrices and vectors that are necessary for the transitions from
///    hidden layer 1 to hidden layer n.
/// + `output_layer` contains the weight matrix and bias vector to transition to the output neurons.
/// + `min_input`, `max_input`, `mean_value` and `range` are necessary to perform input normalization.
/// + `mean_output` and `range_output` are used to undo normalization for output values.
///
/// For more information on that, read up [here](https://github.com/sisl/nnet).
#[derive(Debug)]
pub struct NNet<
    const N_INPUT: usize,
    const N_MAT: usize,
    const N_NEURON: usize,
    const N_OUTPUT: usize,
> {
    pub input_layer: Layer<N_INPUT, N_NEURON>,
    pub hidden_layers: [Layer<N_NEURON, N_NEURON>; N_MAT],
    pub output_layer: Layer<N_NEURON, N_OUTPUT>,
    pub min_input: Vector<N_INPUT>,
    pub max_input: Vector<N_INPUT>,
    pub mean_value: Vector<N_INPUT>,
    pub range: Vector<N_INPUT>,
    pub mean_output: f32,
    pub range_output: f32,
}

/// One layer of a neuronal network, consisting of a matrix of weights and a vector of biases.
///
/// + The matrix is of the dimension `OUTPUT_NEURONS` rows x `INPUT_NEURONS` columns
/// + The vector is of the dimension `OUTPUT_NEURONS`
#[derive(Debug, Clone, Copy)]
pub struct Layer<const INPUT_NEURONS: usize, const OUTPUT_NEURONS: usize> {
    pub a: Matrix<OUTPUT_NEURONS, INPUT_NEURONS>,
    pub biases: Vector<OUTPUT_NEURONS>,
}

impl<const N_INPUT: usize, const N_MAT: usize, const N_NEURON: usize, const N_OUTPUT: usize>
    NNet<N_INPUT, N_MAT, N_NEURON, N_OUTPUT>
{
    /// Evaluates a neuronal network with specific inputs:
    ///
    /// The inputs will be normalized (see `normalize()`) and then processed.
    /// This is basically a lot of linear algebra. It breaks down to
    /// `y = m * x + t` done by any given neuron in a given layer. Because of the amount of neurons,
    /// it is easy to do via matrix and vector multiplication and addition.
    /// ´Undo_normalize()` will reverse the normalization so the result becomes more interpretable.
    pub fn eval(&self, mut inputs: Vector<N_INPUT>) -> Vector<N_OUTPUT> {
        self.normalize(&mut inputs);

        //println!("Inputs after normalization: {:?}", &inputs);

        //Doing the actual network evaluation
        let mut accumulator =
            (self.input_layer.a * inputs + self.input_layer.biases).sup(&Vector::zeros());
        //println!("After the input layer: {:?}", &accumulator);

        for layer in &self.hidden_layers {
            accumulator = (layer.a * accumulator + layer.biases).sup(&Vector::zeros());
            //println!("Updates within the hidden layers: {:?}", &accumulator);
        }

        let mut output = self.output_layer.a * accumulator + self.output_layer.biases;
        //println!("Just after output layer (not normalized): {:?}", &output);

        self.undo_normalize(&mut output);
        //println!("Now undone normalize: {:?}", &output);

        output
    }

    /// Normalize network inputs:
    ///
    /// The network can only function for values in between -1 and 1.
    /// The input values will be compared to the maximum and minimum value.
    /// If they are too low, the lower bound will be used, same thing with the upper bound.
    /// If they are within the range, the actual value will be use.
    /// Normalization will be done by subtracting the mean value and dividing the result by the value range.
    fn normalize(&self, inputs: &mut Vector<N_INPUT>) {
        *inputs = inputs
            .sup(&self.min_input)
            .inf(&self.max_input)
            .map_with_location(|i, _, e| (e - self.mean_value[i]) / self.range[i]);
    }

    /// Undo normalization on network outputs:
    ///
    /// This reverses the normalization for all network outputs and makes the result interpredable.
    fn undo_normalize(&self, inputs: &mut Vector<N_OUTPUT>) {
        *inputs = (*inputs * self.range_output).add_scalar(self.mean_output)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use nalgebra::{matrix, vector};

    #[test]
    fn basic_inference_example() {
        let nnet = NNet {
            input_layer: Layer {
                a: matrix![
                    2.0, 3.0;
                    1.0, 2.0
                ],
                biases: vector![5.0, 3.0],
            },
            hidden_layers: [
                // layer 1
                Layer {
                    a: matrix![
                        2.0, 3.0;
                        1.0, 2.0
                    ],
                    biases: vector![5.0, 3.0],
                },
                // layer 2
                Layer {
                    a: matrix![
                        2.0, 3.0;
                        1.0, 2.0
                    ],
                    biases: vector![5.0, 3.0],
                },
            ],
            output_layer: Layer {
                a: matrix![
                    2.0, 3.0;
                    1.0, 2.0
                ],
                biases: vector![5.0, 3.0],
            },
            min_input: vector![-2.0, -2.5],
            max_input: vector![2.0, 2.5],
            mean_value: vector![0.0, 0.0],
            range: vector![4.0, 5.0],
            mean_output: 1.5,
            range_output: 5.0,
        };

        // normlaization by hand mit [1.3, 2.3] und obigen Werten
        let input = vector![1.3, 2.3];

        let output = nnet.eval(input);

        assert_eq!(output, vector![2345.525, 1355.6001]);
    }
}
