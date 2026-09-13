use std::io::{self, Write};

use anyhow::Context;
use flexi_logger::{
    Age, Cleanup, Criterion, DeferredNow, Duplicate, FileSpec, Logger, LoggerHandle, Naming,
};
use log::Record;

use crate::config::{LogOutput, LoggingConfig};

const LOG_FILE_BASENAME: &str = "xiaomi-scale-mcp";
const LOG_DATE_FORMAT: &str = "%Y-%m-%d";
const LOG_TIMESTAMP_FORMAT: &str = "%Y-%m-%d %H:%M:%S%.3f";

pub(crate) fn initialize(config: &LoggingConfig) -> anyhow::Result<Option<LoggerHandle>> {
    configured_logger(config)?
        .map(|logger| logger.start().context("failed to initialize logging"))
        .transpose()
}

fn configured_logger(config: &LoggingConfig) -> anyhow::Result<Option<Logger>> {
    if !config.enabled {
        return Ok(None);
    }

    let logger = Logger::try_with_env_or_str(config.level.as_str())
        .context("failed to configure logging")?
        .format(timestamped_format);
    let logger = match config.output {
        LogOutput::Console => logger.log_to_stderr(),
        LogOutput::File => configure_file_output(logger, config),
        LogOutput::Both => {
            configure_file_output(logger, config).duplicate_to_stderr(Duplicate::All)
        }
    };

    Ok(Some(logger))
}

fn timestamped_format(
    writer: &mut dyn Write,
    now: &mut DeferredNow,
    record: &Record<'_>,
) -> io::Result<()> {
    write!(
        writer,
        "[{}] {} [{}] {}",
        now.format(LOG_TIMESTAMP_FORMAT),
        record.level(),
        record.module_path().unwrap_or("<unnamed>"),
        record.args()
    )
}

fn configure_file_output(logger: Logger, config: &LoggingConfig) -> Logger {
    logger
        .log_to_file(
            FileSpec::default()
                .directory(config.directory.as_path())
                .basename(LOG_FILE_BASENAME),
        )
        .rotate(
            Criterion::Age(Age::Day),
            Naming::TimestampsCustomFormat {
                current_infix: None,
                format: LOG_DATE_FORMAT,
            },
            Cleanup::Never,
        )
        .append()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LogOutput;

    #[test]
    fn disabled_logging_does_not_build_a_logger() {
        let config = LoggingConfig {
            enabled: false,
            ..LoggingConfig::default()
        };

        assert!(configured_logger(&config).unwrap().is_none());
    }

    #[test]
    fn builds_a_logger_for_every_output() {
        for output in [LogOutput::Console, LogOutput::File, LogOutput::Both] {
            let config = LoggingConfig {
                output,
                ..LoggingConfig::default()
            };

            assert!(configured_logger(&config).unwrap().is_some());
        }
    }
}
