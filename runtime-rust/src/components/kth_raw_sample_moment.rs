use whitenoise_validator::errors::*;

use crate::base::NodeArguments;
use whitenoise_validator::base::{Value, get_argument, ArrayND};
use crate::components::Evaluable;
use whitenoise_validator::proto;
use ndarray::ArrayD;
use crate::utilities::utilities::get_num_columns;

impl Evaluable for proto::KthRawSampleMoment {
    fn evaluate(&self, arguments: &NodeArguments) -> Result<Value> {
        let data = get_argument(&arguments, "data")?.get_arraynd()?.get_f64()?;
        let k = get_argument(&arguments, "k")?.get_first_i64()?;
        Ok(Value::ArrayND(ArrayND::F64(kth_raw_sample_moment(&data, &k))))
    }
}


/// Accepts data and returns sample estimate of kth raw moment
///
/// # Arguments
/// * `data` - Array of data for which you would like the kth raw moment
/// * `k` - integer representing moment you want
///
/// # Return
/// kth sample moment
///
/// # Example
/// ```
/// use ndarray::prelude::*;
/// use whitenoise_runtime::utilities::aggregations::kth_raw_sample_moment;
/// let data: ArrayD<f64> = arr1(&[0., 1., 2., 3., 4., 5., 12., 19., 24., 90., 98., 100.]).into_dyn();
/// let third_moment: ArrayD<f64> = kth_raw_sample_moment(&data, &3);
/// println!("{}", third_moment);
/// ```
pub fn kth_raw_sample_moment(data: &ArrayD<f64>, k: &i64) -> Result<ArrayD<f64>> {
    
    let mut data = data.clone();

    let num_columns = get_num_columns(&data)?;

    // iterate over the generalized columns
    data.gencolumns_mut().into_iter()
        // for each pairing, iterate over the cells
        .for_each(|mut column| column.iter_mut()
            // mutate the cell via the operator
            .for_each(|v| *v = v.powi(k)));

    Ok(data)
    let data_vec: Vec<f64> = data.clone().into_dimensionality::<Ix1>().unwrap().to_vec();
    let data_to_kth_power: Vec<f64> = data_vec.iter().map(|x| x.powf(*k as f64)).collect();
    return mean(&arr1(&data_to_kth_power).into_dyn());
}