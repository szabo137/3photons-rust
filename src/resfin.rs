//! This module contains everything that is needed to compute, store, and
//! analyze the final results: differential cross-section, sum & variance

use ::{
    config::Configuration,
    event::{INCOMING_COUNT, OUTGOING_COUNT},
    numeric::{
        Complex,
        functions::{abs, powi, sqr, sqrt},
        Real,
        reals::consts::PI,
    },
    rescont::{ResultContribution, ResultVector},
};

use num_traits::Zero;


/// Number of spins
const NUM_SPINS: usize = 2;


/// This struct will accumulate intermediary results during integration, and
/// ultimately compute the final results (see FinalResults below).
pub struct ResultsBuilder<'a> {
    // ### RESULT ACCUMULATORS ###

    /// Number of integrated events
    selected_events: i32,

    /// Accumulated cross-section for each contribution
    spm2: ResultVector<Real>,

    /// Accumulated variance for each contribution
    vars: ResultVector<Real>,

    /// Impact of each contribution on the cross-section
    sigma_contribs: ResultVector<Real>,

    /// Accumulated total cross-section
    sigma: Real,

    /// Accumulated total variance
    variance: Real,


    // ### PHYSICAL CONSTANTS (CACHED FOR FINALIZATION) ###

    /// Configuration of the simulation
    config: &'a Configuration,

    /// Common factor, non-averaged over spins=1
    ///                  /(symmetry factor)
    ///                  *(conversion factor GeV^-2->pb)
    /// To average over spins, add :
    ///                  /(number of incoming helicities)
    fact_com: Real,

    /// Event weight, with total phase space normalization
    norm_weight: Real,

    /// Z° propagator
    propag: Real,

    /// ??? (Ask Vincent Lafage)
    ecart_pic: Real,
}
//
impl<'a> ResultsBuilder<'a> {
    // Prepare for results integration
    pub fn new(cfg: &'a Configuration, event_weight: Real) -> Self {
        // Common factor (see definition and remarks above)
        let fact_com = 1./6. * cfg.convers;
        let gzr = cfg.g_z0 / cfg.m_z0;

        // Sum over polarisations factors
        let p_aa = 2.;
        let p_ab = 1. - 4.*cfg.sin2_w;
        let p_bb = p_ab + 8.*sqr(cfg.sin2_w);

        // Homogeneity coefficient
        let c_aa = fact_com * p_aa;
        let c_ab = fact_com * p_ab / sqr(cfg.m_z0);
        let c_bb = fact_com * p_bb / powi(cfg.m_z0, 4);

        // Switch to dimensionless variable
        let dzeta = sqr(cfg.e_tot / cfg.m_z0);
        let ecart_pic = (dzeta - 1.) / gzr;
        let propag = 1. / (1. + sqr(ecart_pic));

        // Apply total phase space normalization to the event weight
        let n_ev = cfg.num_events as Real;
        let norm = powi(2.*PI, 4-3*(OUTGOING_COUNT as i32)) / n_ev;
        // NOTE: This replaces the original WTEV, previously reset every event
        let norm_weight = event_weight * norm;

        // Compute how much each result contribution adds to the cross-section.
        // Again, this avoids duplicate work in the integration loop.
        let com_contrib = norm_weight / 4.;
        let aa_contrib = com_contrib * c_aa;
        let bb_contrib = com_contrib * c_bb * propag / sqr(gzr);
        let ab_contrib = com_contrib * c_ab * 2. * cfg.beta_plus * propag / gzr;
        let sigma_contribs = ResultVector::new(aa_contrib,
                                               bb_contrib * sqr(cfg.beta_plus),
                                               bb_contrib * sqr(cfg.beta_minus),
                                               ab_contrib * ecart_pic,
                                               -ab_contrib);

        // Return a complete results builder
        ResultsBuilder {
            selected_events: 0,
            spm2: ResultVector::zero(),
            vars: ResultVector::zero(),
            sigma_contribs: sigma_contribs,
            sigma: 0.,
            variance: 0.,

            config: cfg,
            fact_com: fact_com,
            norm_weight: norm_weight,
            propag: propag,
            ecart_pic: ecart_pic,
        }
    }

    /// Integrate one intermediary result into the simulation results
    pub fn integrate(&mut self, result: ResultContribution) {
        self.selected_events += 1;
        let spm2_dif = result.m2_sums();
        self.spm2 += spm2_dif;
        self.vars += spm2_dif.map(sqr);
        let weight = spm2_dif.dot(&self.sigma_contribs);
        self.sigma += weight;
        self.variance += sqr(weight);
    }

    // After integration is done, build final simulations results
    pub fn finalize(self) -> FinalResults<'a> {
        // Extract whatever our needs from the results builder
        let cfg = self.config;

        // Compute the final results
        let mut results = FinalResults {
            selected_events: self.selected_events,
            spm2: [self.spm2, ResultVector::zero()],
            vars: [self.vars, ResultVector::zero()],
            sigma: self.sigma,
            variance: self.variance,
            beta_min: 0.,
            prec: 0.,
            ss_p: 0.,
            inc_ss_p: 0.,
            ss_m: 0.,
            inc_ss_m: 0.,
            config: cfg,
        };
        {
            // Borrow spm2 and var from them to avoid repetitive member access
            let spm2 = &mut results.spm2;
            let var = &mut results.vars;

            // Keep around a floating-point version of the total event count
            let n_ev = cfg.num_events as Real;

            // Compute the relative uncertainties for one spin
            for (&v_spm2, v_var) in spm2[0].iter().zip(var[0].iter_mut()) {
                *v_var = (*v_var - sqr(v_spm2)/n_ev) / (n_ev - 1.);
                *v_var = sqrt(*v_var/n_ev) / abs(v_spm2/n_ev);
            }

            // Copy for the opposite spin
            spm2[1] = spm2[0];
            var[1] = var[0];

            // Electroweak polarisations factors for the 𝛽₊/𝛽₋ anomalous
            // contribution
            let pol_p = -2. * cfg.sin2_w;
            let pol_m = 1. + pol_p;

            // Take polarisations into account
            for k in 1..3 {
                spm2[0][k] *= sqr(pol_m);
                spm2[1][k] *= sqr(pol_p);
            }
            for k in 3..5 {
                spm2[0][k] *= pol_m;
                spm2[1][k] *= pol_p;
            }

            // Flux factor (=1/2s for 2 initial massless particles)
            let flux = 1. / (2. * sqr(cfg.e_tot));

            // Apply physical coefficients and Z⁰ propagator to each spin
            let gz0_mz0 = cfg.g_z0 * cfg.m_z0;
            for s_spm2 in spm2.iter_mut() {
                for v_spm2 in s_spm2.iter_mut() {
                    *v_spm2 *= self.fact_com * flux * self.norm_weight;
                }
                s_spm2[0] *= 1.;
                s_spm2[1] *= self.propag / sqr(gz0_mz0);
                s_spm2[2] *= self.propag / sqr(gz0_mz0);
                s_spm2[3] *= self.propag * self.ecart_pic / gz0_mz0;
                s_spm2[4] *= self.propag / gz0_mz0;
            }

            results.beta_min = sqrt((spm2[0][0]+spm2[1][0]) /
                                        (spm2[0][1]+spm2[1][1]));

            let ss_denom = spm2[0][0] + spm2[1][0];

            results.ss_p = (spm2[0][1] + spm2[1][1]) / (2. * sqrt(ss_denom));
            results.ss_m = (spm2[0][2] + spm2[1][2]) / (2. * sqrt(ss_denom));

            let inc_ss_common = sqrt(sqr(spm2[0][0]*var[0][0])
                                     + sqr(spm2[1][0]*var[1][0])) /
                                (2. * abs(ss_denom));

            results.inc_ss_p = sqrt(sqr(spm2[0][1]*var[0][1])
                                    + sqr(spm2[1][1]*var[1][1])) /
                               abs(spm2[0][1] + spm2[1][1])
                             + inc_ss_common;
            results.inc_ss_m = sqrt(sqr(spm2[0][2]*var[0][2])
                                    + sqr(spm2[1][2]*var[1][2])) /
                               abs(spm2[0][2] + spm2[1][2])
                             + inc_ss_common;

            results.variance = (self.variance - sqr(self.sigma)/n_ev) /
                                   (n_ev-1.);
            results.prec = sqrt(results.variance/n_ev) / abs(self.sigma/n_ev);
            results.sigma = self.sigma * flux;
        }

        // Return the final results
        results
    }
}


/// This struct will hold the final results of the simulation
pub struct FinalResults<'a> {
    /// Number of integrated events
    pub selected_events: i32,

    /// Cross-section for each spin
    pub spm2: [ResultVector<Real>; NUM_SPINS],

    /// Variance for each spin
    pub vars: [ResultVector<Real>; NUM_SPINS],

    /// Total cross-section
    pub sigma: Real,

    /// Relative precision
    pub prec: Real,

    /// Total variance
    pub variance: Real,

    /// Beta minimum (???)
    pub beta_min: Real,

    /// Statistical significance B+(pb-1/2) (???) and associated incertitude
    pub ss_p: Real,
    pub inc_ss_p: Real,

    /// Statistical significance B-(pb-1/2) (???) and associated incertitude
    pub ss_m: Real,
    pub inc_ss_m: Real,

    /// Configuration of the simulation (for further derivation)
    config: &'a Configuration,
}
//
impl<'a> FinalResults<'a> {
    /// Display results using Eric's (???) parametrization
    pub fn eric(&self) {
        assert_eq!(INCOMING_COUNT, 2);
        assert_eq!(OUTGOING_COUNT, 3);

        let cfg = self.config;

        let mu_th = cfg.br_ep_em * cfg.convers /
            (8. * 9. * 5. * sqr(PI) * cfg.m_z0 * cfg.g_z0);
        let lambda0_m = (-self.spm2[0][1] + self.spm2[0][2]) / 2.;
        let lambda0_p = (-self.spm2[1][1] + self.spm2[1][2]) / 2.;
        let mu0_m = (self.spm2[0][1] + self.spm2[0][2]) / 2.;
        let mu0_p = (self.spm2[1][1] + self.spm2[1][2]) / 2.;
        let mu_num = (self.spm2[0][1] +
                      self.spm2[0][2] +
                      self.spm2[1][1] +
                      self.spm2[1][2]) / 4.;

        println!();
        println!("       :        -          +");
        println!("sigma0  : {:.6} | {:.6}",
                 self.spm2[0][0]/2.,
                 self.spm2[1][0]/2.);
        println!("alpha0  : {:.5e} | {:.4e}",
                 self.spm2[0][4]/2.,
                 self.spm2[1][4]/2.);
        println!("beta0   : {:} | {:}",
                 -self.spm2[0][3]/2.,
                 -self.spm2[1][3]/2.);
        println!("lambda0 : {:.4} | {:.4}", lambda0_m, lambda0_p);
        println!("mu0     : {:.4} | {:.5}", mu0_m, mu0_p);
        println!("mu/lamb : {:.5} | {:.5}", mu0_m/lambda0_m, mu0_p/lambda0_p);
        println!("mu (num): {:.4}", mu_num);
        println!("rapport : {:.6}", mu_num/mu_th);
        println!("mu (th) : {:.4}", mu_th);
    }

    /// Display Fawzi's (???) analytical results and compare them to the Monte
    /// Carlo results that we have computed
    pub fn fawzi(&self) {
        assert_eq!(INCOMING_COUNT, 2);
        assert_eq!(OUTGOING_COUNT, 3);

        let cfg = self.config;
        let ref ev_cut = cfg.event_cut;

        let mre = cfg.m_z0 / cfg.e_tot;
        let gre = cfg.g_z0 * cfg.m_z0 / sqr(cfg.e_tot);
        let x = 1. - sqr(mre);
        let sdz = Complex::new(x, -gre) / (sqr(x) + sqr(gre));
        let del = (1. - ev_cut.b_cut) / 2.;
        let eps = 2. * ev_cut.e_min / cfg.e_tot;
        let bra = cfg.m_z0 / (3. * 6. * powi(PI, 3) * 16. * 120.);
        let sig = 12. * PI / sqr(cfg.m_z0) * cfg.br_ep_em *
            cfg.g_z0 * bra / sqr(cfg.e_tot) *
            powi(cfg.e_tot / cfg.m_z0, 8) * sdz.norm_sqr() * 
            cfg.convers;

        let eps_4 = powi(eps, 4);
        let del_2 = powi(del, 2);
        let del_3 = powi(del, 3);
        let f1 =       1. - 15. * eps_4
            - 9./7. * (1. - 70. * eps_4) * del_2
            + 6./7. * (1. + 70. * eps_4) * del_3;
        let g1 =       1. -  30. * eps_4
            - 9./7. * (1. -  70. * eps_4) * del
            - 90.                * eps_4  * del_2
            - 1./7. * (1. - 420. * eps_4) * del_3;
        let g2 =        1. -  25. * eps_4
            - 6./ 7. * (1. -  70. * eps_4) * del
            - 3./ 7. * (1. + 210. * eps_4) * del_2
            - 8./21. * (1. - 52.5 * eps_4) * del_3;
        let g3 =        1.    - 195./11. * eps_4
            -18./77. * (1.    -       7. * eps_4) * del
            - 9./11. * (9./7. -      70. * eps_4) * del_2
            - 8./11. * (1.    - 105./11. * eps_4) * del_3;

        let sincut_3 = powi(ev_cut.sin_cut, 3);
        let ff = f1 * (1. - sincut_3);
        let gg = g1 - 27./16.*g2*ev_cut.sin_cut + 11./16.*g3*sincut_3;

        let sig_p = sig*(ff+2.*gg);
        let sig_m = sig_p + 2.*sig*gg;

        let mc_p = (self.spm2[0][1] + self.spm2[1][1]) / 4.;
        let mc_m = (self.spm2[0][2] + self.spm2[1][2]) / 4.;
        let incr_p = sqrt(sqr(self.spm2[0][1] * self.vars[0][1]) +
                            sqr(self.spm2[1][1] * self.vars[1][1]))
                     / abs(self.spm2[0][1] + self.spm2[1][1]);
        let incr_m = sqrt(sqr(self.spm2[0][2] * self.vars[0][2]) +
                            sqr(self.spm2[1][2] * self.vars[1][2]))
                     / abs(self.spm2[0][2] + self.spm2[1][2]);

        println!();
        println!("s (pb) :   Sig_cut_Th    Sig_Th      Rapport");
        println!("       :   Sig_Num");
        println!("       :   Ecart_relatif  Incertitude");
        println!();
        println!("s+(pb) : {:.5} | {:.5} | {:.6}",
                 sig_p,
                 sig*3.,
                 sig_p/(3.*sig));
        println!("       : {:.5}", mc_p);
        println!("       : {:.6} | {:.8} | {:.2}",
                 mc_p/sig_p - 1.,
                 incr_p,
                 (mc_p/sig_p - 1.) / incr_p);
        println!();
        println!("s-(pb) : {:.5} | {:.4} | {:.6}",
                 sig_m,
                 sig*5.,
                 sig_m/(5.*sig));
        println!("       : {:.5}", mc_m);
        println!("       : {:.6} | {:.9} | {:.2}",
                 mc_m/sig_m - 1.,
                 incr_m,
                 (mc_m/sig_m - 1.) / incr_m);
        println!();
    }
}
