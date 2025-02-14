//! Mechanism for loading and sharing the simulation configuration

use crate::{evcut::EventCut, numeric::Float, Result};
use anyhow::{ensure, format_err, Context, Error};
use std::{fmt::Display, fs::File, io::Read, str::FromStr};

/// Simulation configuration
pub struct Configuration {
    /// Number of events to be simulated
    pub num_events: usize,

    /// Collision energy at center of mass (GeV)
    pub e_total: Float,

    /// Cuts on the angles and energies of generated photons
    pub event_cut: EventCut,

    /// Fine structure constant
    pub alpha: Float,

    /// Fine structure constant at the Z⁰ mass peak
    pub alpha_z: Float,

    /// Conversion factor from GeV^(-2) to pb
    pub gev2_to_picobarn: Float,

    /// Z⁰ boson mass (GeV)
    pub m_z0: Float,

    /// Z⁰ boson width (GeV)
    pub g_z0: Float,

    /// Square sine of Weinberg's Theta
    pub sin2_weinberg: Float,

    /// Branching ratio from Z to e+/e-
    pub branching_ep_em: Float,

    /// Beta + (???)
    pub beta_plus: Float,

    /// Beta - (???)
    pub beta_minus: Float,

    /// Number of histogram bins (UNUSED)
    num_bins: i32,

    /// Whether intermediary results should be displayed (UNUSED)
    impr: bool,

    /// Whether results should be plotted in a histogram (UNUSED)
    plot: bool,
}
//
impl Configuration {
    /// Load the configuration from a file, check it, and print it out
    pub fn load(file_name: &str) -> Result<Self> {
        // Read out the simulation's configuration file or die trying.
        let config_str = {
            let mut config_file = File::open(file_name)?;
            let mut buffer = String::new();
            config_file.read_to_string(&mut buffer)?;
            buffer
        };

        // We will iterate over the configuration items. In 3photons' simple
        // config file format, these should be the first non-whitespace chunk of
        // text on each line. We will ignore blank lines.
        let mut config_iter = config_str
            .lines()
            .filter_map(|line| line.split_whitespace().next());

        // This closure fetches the next configuration item, tagging it with
        // the name of the configuration field which it is supposed to fill to
        // ease error reporting, and handling unexpected end-of-file too.
        let mut next_item = |name: &'static str| -> Result<ConfigItem> {
            config_iter
                .next()
                .map(|data| ConfigItem::new(name, data))
                .ok_or_else(|| format_err!("Missing configuration of {}", name))
        };

        // Decode the configuration items into concrete values
        let config = Configuration {
            num_events: next_item("num_events")?.parse::<usize>()?,
            e_total: next_item("e_total")?.parse::<Float>()?,
            event_cut: EventCut::new(
                next_item("beam_photons_cut")?.parse::<Float>()?,
                next_item("photon_photon_cut")?.parse::<Float>()?,
                next_item("e_min")?.parse::<Float>()?,
                next_item("beam_photon_plane_cut")?.parse::<Float>()?,
            ),
            alpha: next_item("alpha")?.parse::<Float>()?,
            alpha_z: next_item("alpha_z")?.parse::<Float>()?,
            gev2_to_picobarn: next_item("gev2_to_picobarn")?.parse::<Float>()?,
            m_z0: next_item("m_z0")?.parse::<Float>()?,
            g_z0: next_item("g_z0")?.parse::<Float>()?,
            sin2_weinberg: next_item("sin2_weinberg")?.parse::<Float>()?,
            branching_ep_em: next_item("branching_ep_em")?.parse::<Float>()?,
            beta_plus: next_item("beta_plus")?.parse::<Float>()?,
            beta_minus: next_item("beta_moins")?.parse::<Float>()?,
            num_bins: next_item("num_bins")?.parse::<i32>()?,
            impr: next_item("impr")?.parse_bool()?,
            plot: next_item("plot")?.parse_bool()?,
        };

        // Display it the way the C++ version used to (this eases comparisons)
        print!("{config}");

        // A sensible simulation must run for at least one event
        ensure!(config.num_events > 0, "Please simulate at least one event");

        // We don't support the original code's PAW-based plotting features,
        // so we make sure that it was not enabled.
        ensure!(!config.plot, "Plotting is not supported by this version");

        // We do not support the initial code's debugging feature which displays
        // all intermediary results during sampling. Such a feature should be
        // set up at build time to avoid run-time costs.
        ensure!(
            !config.impr,
            "Individual result printing is not supported. This debugging feature has a run-time \
             performance cost even when unused. It should be implemented at compile-time instead."
        );

        // If nothing bad occured, we can now return the configuration
        Ok(config)
    }
}

impl Display for Configuration {
    /// Display the configuration, following formatting of the original version
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(fmt, "ITOT           : {}", self.num_events)?;
        writeln!(fmt, "ETOT           : {}", self.e_total)?;
        writeln!(fmt, "oCutpar.ACUT   : {}", self.event_cut.beam_photons_cut)?;
        writeln!(fmt, "oCutpar.BCUT   : {}", self.event_cut.photon_photon_cut)?;
        writeln!(fmt, "oCutpar.EMIN   : {}", self.event_cut.e_min)?;
        let beam_phpl_cut = self.event_cut.beam_photon_plane_cut;
        writeln!(fmt, "oCutpar.SINCUT : {beam_phpl_cut}")?;
        writeln!(fmt, "ALPHA          : {}", self.alpha)?;
        writeln!(fmt, "ALPHAZ         : {}", self.alpha_z)?;
        writeln!(fmt, "CONVERS        : {}", self.gev2_to_picobarn)?;
        writeln!(fmt, "oParam.MZ0     : {}", self.m_z0)?;
        writeln!(fmt, "oParam.GZ0     : {}", self.g_z0)?;
        writeln!(fmt, "SIN2W          : {}", self.sin2_weinberg)?;
        writeln!(fmt, "BREPEM         : {}", self.branching_ep_em)?;
        writeln!(fmt, "BETAPLUS       : {}", self.beta_plus)?;
        writeln!(fmt, "BETAMOINS      : {}", self.beta_minus)?;
        writeln!(fmt, "NBIN           : {}", self.num_bins)?;
        writeln!(fmt, "oParam.IMPR    : {}", self.impr)?;
        writeln!(fmt, "PLOT           : {}", self.plot)?;
        Ok(())
    }
}

/// A value from the configuration file, tagged with the struct field which it
/// is supposed to map for error reporting purposes.
struct ConfigItem<'data> {
    name: &'static str,
    data: &'data str,
}
//
impl<'data> ConfigItem<'data> {
    /// Build a config item from a struct field tag and raw iterator data
    fn new(name: &'static str, data: &'data str) -> Self {
        Self { name, data }
    }

    /// Parse this data using Rust's standard parsing logic
    fn parse<T: FromStr>(self) -> Result<T>
    where
        <T as FromStr>::Err: ::std::error::Error + Send + Sync + 'static,
    {
        self.data
            .parse::<T>()
            .map_err(Error::new)
            .context(format!("Could not parse configuration of {}", self.name))
    }

    /// Parse this data using special logic which handles Fortran's bool syntax
    //
    // TODO: Once Rust has specialization, try to make parse_bool a special case
    //       of parse that's invoked for bool arguments, and ideally use that to
    //       simplify the caller code to just a call to parse().
    //
    fn parse_bool(self) -> Result<bool> {
        match self.data.to_lowercase().as_str() {
            // Handle FORTRAN booleans as a special case
            ".true." => Ok(true),
            ".false." => Ok(false),
            // Delegate other booleans to the standard Rust parser
            _ => self.parse::<bool>(),
        }
    }
}
