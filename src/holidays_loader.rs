

fn load_holidays(path:&str) {
    let mut rdr = csv::Reader::from_path(path);
    for result in rdr.records() {
        // The iterator yields Result<StringRecord, Error>, so we check the
        // error here.
        let record = result.unwrap();
        println!("{:?}", record);
    }
}