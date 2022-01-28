use nalgebra::base::{SMatrix, SVector};

pub type Matrix<const ROWS: usize, const COLS: usize> = SMatrix<f32, ROWS, COLS>;
pub type Vector<const ROWS: usize> = SVector<f32, ROWS>;

/// + `N_MAT` is the number of matrices transforming betweent the hidden layers, so if there are
///   `n` hidden layers `N_MAT == n - 1`
pub struct NNet<
    const N_INPUT: usize,
    const N_MAT: usize,
    const N_NEURON: usize,
    const N_OUTPUT: usize,
> {
    // input -> first hidden layer: Matr 50 zeilen, 7 spalten
    // hiddenlay_n -> hidden_layer_n+1 : Matrix 50x50
    // last hidden_layer -> output: Matrix 50 zeilen, 5 spalte
    //
    // for each step: biases addieren
    input_weights: Matrix<N_NEURON, N_INPUT>,
    input_biases: Vector<N_INPUT>,
    hidden_layers: [Layer<N_NEURON>; N_MAT],
    output_weights: Matrix<N_OUTPUT, N_NEURON>,
    // TODO output biases?
}

pub struct Layer<const N: usize> {
    a: Matrix<N, N>,
    weights: Vector<N>,
}

impl<const N_INPUT: usize, const N_MAT: usize, const N_NEURON: usize, const N_OUTPUT: usize>
    NNet<N_INPUT, N_MAT, N_NEURON, N_OUTPUT>
{
    /// Evaluates the given NNet with specific inputs
    pub fn eval(&self, inputs: &Vector<N_INPUT>) -> Vector<N_OUTPUT> {
        // TODO normalize inputs

        // TODO check the actual core of the algorithm, is it correct?
        //
        // adding `self.input_biases` doesn't work, as input_weights * x is a vector of N_NEURON
        // elements, but input weights is a vector of N_INPUT elements
        let mut accumulator = self.input_weights * inputs;

        for layer in &self.hidden_layers {
            accumulator = layer.a * accumulator + layer.weights;
        }

        let output = self.output_weights * accumulator;

        // TODO normalize outputs

        output
    }
}

pub struct Neuron {
    weight: f32,
    bias: f32,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn first_test() {
        let my_vec: Vector<3> = Vector::from_column_slice(&[1.0, 2.0, 3.0]);

        let my_mat: Matrix<2, 3> = Matrix::from_row_slice(&[
            1.0, 2.0, // first row
            3.0, 4.0, // second row
            1.0, 8.0,
        ]); // third row

        panic!("Oh no! Just kidding, we hijack the fact that panic prints are outputted on test runs to print from a test:\n{:?}", my_mat * my_vec);
    }
}
