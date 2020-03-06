//! This module contains everything that is needed to compute, store, and
//! analyze the final results: differential cross-section, sum & variance

use crate::{
    config::Configuration,
    event::NUM_SPINS,
    linalg::{dimension::*, vecmat::*},
    matelems::{A, B_M, B_P, I_MX, NUM_MAT_ELEMS, R_MX},
    numeric::{functions::*, reals::consts::PI, Complex, Float},
};

/// Matrix of per-spin result contributions
///
/// Rows are spins, columns are result contributions (in the rescont.rs sense)
///
pub type PerSpinMEs = Matrix2x5<Float>;

/// Index of negative spin data
pub const SP_M: usize = 0;

/// Index of positive spin data
pub const SP_P: usize = 1;

/// Final results of the simulation
pub struct FinalResults<'cfg> {
    /// Number of integrated events
    pub selected_events: usize,

    /// Cross-section for each spin
    pub spm2: PerSpinMEs,

    /// Variance for each spin
    pub vars: PerSpinMEs,

    /// Total cross-section
    pub sigma: Float,

    /// Relative precision
    pub prec: Float,

    /// Total variance
    pub variance: Float,

    /// Beta minimum (???)
    pub beta_min: Float,

    /// Statistical significance B+(pb-1/2) (???)
    pub ss_p: Float,

    /// Incertitide associated with ss_p
    pub inc_ss_p: Float,

    /// Statistical significance B-(pb-1/2) (???)
    pub ss_m: Float,

    /// Incertitude associated with ss_m
    pub inc_ss_m: Float,

    /// Configuration of the simulation (for further derivation)
    pub cfg: &'cfg Configuration,
}
//
impl<'cfg> FinalResults<'cfg> {
    /// Display results using Eric's (???) parametrization
    pub fn eric(&self) {
        assert_eq!(NUM_SPINS, 2);
        assert_eq!(NUM_MAT_ELEMS, 5);

        let spm2 = &self.spm2;
        let cfg = self.cfg;

        let mu_th = cfg.br_ep_em * cfg.convers / (8. * 9. * 5. * sqr(PI) * cfg.m_z0 * cfg.g_z0);
        let sigma0 = spm2.column(A) / 2.;
        let alpha0 = spm2.column(I_MX) / 2.;
        let beta0 = -spm2.column(R_MX) / 2.;
        let lambda0 = (spm2.column(B_M) - spm2.column(B_P)) / 2.;
        let mu0 = (spm2.column(B_M) + spm2.column(B_P)) / 2.;
        let mu_num = spm2.fixed_columns::<U2>(B_P).sum() / 4.;

        println!();
        println!("       :        -          +");
        println!("sigma0  : {:.6} | {:.6}", sigma0[SP_M], sigma0[SP_P]);
        println!("alpha0  : {:.5e} | {:.4e}", alpha0[SP_M], alpha0[SP_P]);
        println!("beta0   : {:} | {:}", beta0[SP_M], beta0[SP_P]);
        println!("lambda0 : {:.4} | {:.4}", lambda0[SP_M], lambda0[SP_P]);
        println!("mu0     : {:.4} | {:.5}", mu0[SP_M], mu0[SP_P]);
        println!(
            "mu/lamb : {:.5} | {:.5}",
            mu0[SP_M] / lambda0[SP_M],
            mu0[SP_P] / lambda0[SP_P]
        );
        println!("mu (num): {:.4}", mu_num);
        println!("rapport : {:.6}", mu_num / mu_th);
        println!("mu (th) : {:.4}", mu_th);
    }

    /// Display Fawzi's (???) analytical results and compare them to the Monte
    /// Carlo results that we have computed
    pub fn fawzi(&self) {
        assert_eq!(NUM_SPINS, 2);
        assert_eq!(NUM_MAT_ELEMS, 5);

        let cfg = self.cfg;
        let ev_cut = &cfg.event_cut;
        let spm2 = &self.spm2;
        let vars = &self.vars;

        let mre = cfg.m_z0 / cfg.e_tot;
        let gre = cfg.g_z0 * cfg.m_z0 / sqr(cfg.e_tot);
        let x = 1. - sqr(mre);
        let sdz = Complex::new(x, -gre) / (sqr(x) + sqr(gre));
        let del = (1. - ev_cut.b_cut) / 2.;
        let eps = 2. * ev_cut.e_min / cfg.e_tot;
        let bra = cfg.m_z0 / (3. * 6. * powi(PI, 3) * 16. * 120.);
        let sig = 12. * PI / sqr(cfg.m_z0) * cfg.br_ep_em * cfg.g_z0 * bra / sqr(cfg.e_tot)
            * powi(cfg.e_tot / cfg.m_z0, 8)
            * sdz.norm_sqr()
            * cfg.convers;

        let eps_4 = powi(eps, 4);
        let del_2 = powi(del, 2);
        let del_3 = powi(del, 3);
        let f1 = 1. - 15. * eps_4 - 9. / 7. * (1. - 70. * eps_4) * del_2
            + 6. / 7. * (1. + 70. * eps_4) * del_3;
        let g1 = 1.
            - 30. * eps_4
            - 9. / 7. * (1. - 70. * eps_4) * del
            - 90. * eps_4 * del_2
            - 1. / 7. * (1. - 420. * eps_4) * del_3;
        let g2 = 1.
            - 25. * eps_4
            - 6. / 7. * (1. - 70. * eps_4) * del
            - 3. / 7. * (1. + 210. * eps_4) * del_2
            - 8. / 21. * (1. - 52.5 * eps_4) * del_3;
        let g3 = 1.
            - 195. / 11. * eps_4
            - 18. / 77. * (1. - 7. * eps_4) * del
            - 9. / 11. * (9. / 7. - 70. * eps_4) * del_2
            - 8. / 11. * (1. - 105. / 11. * eps_4) * del_3;

        let sincut_3 = powi(ev_cut.sin_cut, 3);
        let ff = f1 * (1. - sincut_3);
        let gg = g1 - 27. / 16. * g2 * ev_cut.sin_cut + 11. / 16. * g3 * sincut_3;

        let sig_p = sig * (ff + 2. * gg);
        let sig_m = sig_p + 2. * sig * gg;

        let mc_p = spm2.column(B_P).sum() / 4.;
        let mc_m = spm2.column(B_M).sum() / 4.;

        let incr = |col| {
            spm2.column(col).component_mul(&vars.column(col)).norm() / abs(spm2.column(col).sum())
        };
        let incr_p = incr(B_P);
        let incr_m = incr(B_M);

        println!();
        println!("s (pb) :   Sig_cut_Th    Sig_Th      Rapport");
        println!("       :   Sig_Num");
        println!("       :   Ecart_relatif  Incertitude");
        println!();
        println!(
            "s+(pb) : {:.5} | {:.5} | {:.6}",
            sig_p,
            sig * 3.,
            sig_p / (3. * sig)
        );
        println!("       : {:.5}", mc_p);
        println!(
            "       : {:.6} | {:.8} | {:.2}",
            mc_p / sig_p - 1.,
            incr_p,
            (mc_p / sig_p - 1.) / incr_p
        );
        println!();
        println!(
            "s-(pb) : {:.5} | {:.4} | {:.6}",
            sig_m,
            sig * 5.,
            sig_m / (5. * sig)
        );
        println!("       : {:.5}", mc_m);
        println!(
            "       : {:.6} | {:.9} | {:.2}",
            mc_m / sig_m - 1.,
            incr_m,
            (mc_m / sig_m - 1.) / incr_m
        );
        println!();
    }
}
