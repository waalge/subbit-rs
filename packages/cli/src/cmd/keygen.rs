pub fn run() -> anyhow::Result<()> {
    let key = keygen()?;
    println!("{}", hex::encode(key));
    Ok(())
}

pub fn keygen() -> anyhow::Result<[u8; 32]> {
    let mut buf = [0u8; 32];
    getrandom::fill(&mut buf).map_err(|e| anyhow::anyhow!("sys error"))?;
    Ok(buf)
}
