use anyhow::{bail, Context, Result};
use ai_core::job::Action;
use ai_core::store::{Kv, KvSerde};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

/// Execute one job action against the given KV.
pub async fn execute<K: Kv + KvSerde>(action: &Action, kv: &K) -> Result<()> {
    match action {
        Action::Noop => Ok(()),

        Action::KvPut { key, decode, value } => {
            let kb = key.as_bytes();
            match decode.as_str() {
                "utf8" | "raw" => {
                    let s = value
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("value must be string"))?;
                    kv.put(kb, s.as_bytes());
                    Ok(())
                }
                "string" => {
                    let s = value
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("value must be string"))?
                        .to_string();
                    kv.put_t(kb, &s)?;
                    Ok(())
                }
                "u32" => {
                    let n = value
                        .as_u64()
                        .ok_or_else(|| anyhow::anyhow!("value must be number"))?;
                    let n32: u32 = n.try_into().context("out of range")?;
                    kv.put_t(kb, &n32)?;
                    Ok(())
                }
                "u64" => {
                    let n = value
                        .as_u64()
                        .ok_or_else(|| anyhow::anyhow!("value must be number"))?;
                    kv.put_t(kb, &n)?;
                    Ok(())
                }
                other => bail!("unknown decode {}", other),
            }
        }

        Action::KvDel { key } => {
            let _ = kv.delete(key.as_bytes());
            Ok(())
        }

        Action::Exec { cmd, args, timeout_ms } => {
            let mut c = Command::new(cmd);
            if !args.is_empty() {
                c.args(args);
            }
            let fut = c.status();

            if let Some(ms) = timeout_ms {
                let status = timeout(Duration::from_millis(*ms), fut)
                    .await
                    .context("exec timeout")??;
                if !status.success() {
                    bail!("exec exit status {:?}", status.code());
                }
            } else {
                let status = fut.await?;
                if !status.success() {
                    bail!("exec exit status {:?}", status.code());
                }
            }
            Ok(())
        }

        Action::Http { .. } => {
            // Not implemented in this minimal local runtime
            bail!("http action not implemented in runner");
        }
    }
}
