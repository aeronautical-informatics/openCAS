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
#[derive(Debug)]
pub struct Layer<const INPUT_NEURONS: usize, const OUTPUT_NEURONS: usize> {
    pub a: Matrix<OUTPUT_NEURONS, INPUT_NEURONS>,
    pub biases: Vector<OUTPUT_NEURONS>,
}

impl<const N_INPUT: usize, const N_MAT: usize, const N_NEURON: usize, const N_OUTPUT: usize>
    NNet<N_INPUT, N_MAT, N_NEURON, N_OUTPUT>
{
    /// Normalize network (normally within evaluate but for testing it is outside)
    pub fn normalize(&self, inputs: &mut Vector<N_INPUT>) {
        for (iter, element) in inputs.iter_mut().enumerate() {
            match *element {
                //TODO: can dis be ooptimized away?
                x if x < self.min_input[iter] => {
                    (self.min_input[iter] - self.mean_value[iter]) / self.range[iter]
                }
                x if x > self.max_input[iter] => {
                    (self.max_input[iter] - self.mean_value[iter]) / self.range[iter]
                }
                _ => (*element - self.mean_value[iter]) / self.range[iter],
            };
        }
    }

    /*
    for (i, e) in inputs.iter_mut().enumerate(){

        if *e < self.min_input[i] {
            *e = (self.min_input[i] - self.mean_value[i])/self.range[i];
        }

    }*/

    /// Evaluates a neuronal network with specific inputs
    pub fn eval(&self, mut inputs: Vector<N_INPUT>) -> Vector<N_OUTPUT> {
        // TODO normalize inputs
        self.normalize(&mut inputs);

        // TODO check the actual core of the algorithm, is it correct?
        let mut accumulator =
            (self.input_layer.a * &inputs + self.input_layer.biases).sup(&Vector::zeros());

        for layer in &self.hidden_layers {
            accumulator = (layer.a * accumulator + layer.biases).sup(&Vector::zeros());
        }

        let mut output = self.output_layer.a * accumulator + self.output_layer.biases;

        // TODO normalize outputs
        self.undo_normalize(&mut output);

        output
    }

    pub fn undo_normalize(&self, inputs: &mut Vector<N_OUTPUT>) {
        *inputs = (*inputs * self.range_output).add_scalar(self.mean_output)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn first_test() {
        let mut my_vec: Vector<3> = Vector::from_column_slice(&[-1.0, 2.0, 3.0]);
        // assert_eq!(my_vec[0],1.0);

        for i in my_vec.iter_mut() {
            if i < &mut 0.0 {
                *i = 33.3
            }
        }

        assert_eq!(my_vec[0], 33.3);

        let my_mat: Matrix<2, 3> = Matrix::from_row_slice(&[
            1.0, 2.0, 3.0, // first row
            4.0, 1.0, 8.0, // second row
        ]);
        //panic!("Oh no! Just kidding, we hijack the fact that panic prints are outputted on test runs to print from a test:\n{:?}", my_mat * my_vec);
    }
}

//Erklärungscode

/*
struct ConstVec<T, const N:usize>{
    vec : [T; N],
}

impl<T: std::ops::Mul + std::iter::Sum<<T as std::ops::Mul>::Output> + Copy, const N: usize> ConstVec<T, N>{
    pub fn norm(&self)->T{
        self.vec.iter().map(|e| *e * *e).sum()
    }
}
*/
