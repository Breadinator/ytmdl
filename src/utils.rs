/// If all given results are `Ok`, returns `Ok(vec![ok_values])`,
/// else it returns the first error in the `Vec`
#[allow(clippy::missing_errors_doc)]
pub fn reduce_vec_of_results<T, E>(results: Vec<Result<T, E>>) -> Result<Vec<T>, E> {
    let mut out = Vec::with_capacity(results.len());

    for res in results {
        match res {
            Ok(val) => out.push(val),
            Err(err) => return Err(err),
        }
    }

    Ok(out)
}
