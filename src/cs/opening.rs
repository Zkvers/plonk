use crate::transcript::TranscriptProtocol;
use algebra::{curves::PairingEngine, fields::Field};
use ff_fft::DensePolynomial as Polynomial;
use itertools::izip;
use std::marker::PhantomData;

pub struct commitmentOpener<E: PairingEngine> {
    _engine: PhantomData<E>,
}
impl<E: PairingEngine> commitmentOpener<E> {
    pub fn new() -> Self {
        commitmentOpener {
            _engine: PhantomData,
        }
    }

    pub fn compute_opening_polynomials(
        &self,
        transcript: &mut TranscriptProtocol<E>,
        root_of_unity: E::Fr,
        n: usize,
        z_challenge: E::Fr,
        lin_poly: &Polynomial<E::Fr>,
        evaluations: &[E::Fr],
        t_lo: &Polynomial<E::Fr>,
        t_mid: &Polynomial<E::Fr>,
        t_hi: &Polynomial<E::Fr>,
        w_l_poly: &Polynomial<E::Fr>,
        w_r_poly: &Polynomial<E::Fr>,
        w_o_poly: &Polynomial<E::Fr>,
        sigma_1_poly: &Polynomial<E::Fr>,
        sigma_2_poly: &Polynomial<E::Fr>,
        z_poly: &Polynomial<E::Fr>,
    ) -> (Polynomial<E::Fr>, Polynomial<E::Fr>) {
        let mut evaluations = evaluations.to_vec();

        // Compute 1,v, v^2, v^3,..v^7
        let v = transcript.challenge_scalar(b"v");
        let mut v_pow: Vec<E::Fr> = Vec::with_capacity(6);
        v_pow.push(E::Fr::one());
        for i in 1..9 {
            v_pow[i] = v_pow[i - 1] * &v;
        }

        let v_7 = v_pow.pop().unwrap();
        let z_eval = evaluations.pop().unwrap(); // XXX: For better readability, we should probably have an evaluation struct. It is a vector so that we can iterate in compute_challenge_poly_eval

        // Compute z^n , z^2n
        let z_n = z_challenge.pow(&[n as u64]);
        let z_two_n = z_challenge.pow(&[2 * n as u64]);

        let shifted_z = z_challenge * &root_of_unity;

        let quotient_open_poly =
            self.compute_quotient_opening_poly(t_lo, t_mid, t_hi, z_n, z_two_n);
        let polynomials = vec![
            &quotient_open_poly,
            lin_poly,
            w_l_poly,
            w_r_poly,
            w_o_poly,
            sigma_1_poly,
            sigma_2_poly,
        ];

        // Compute opening polynomial
        let k = self.compute_challenge_poly_eval(v_pow, polynomials, evaluations);

        // Compute W_z(X)
        let W_z = self.compute_witness_polynomial(&k, z_challenge);

        // Compute shifted polynomial
        let W_zw = self.compute_shifted_polynomial(v_7, z_poly, z_eval, shifted_z);

        (W_z, W_zw)
    }

    fn compute_quotient_opening_poly(
        &self,
        t_lo: &Polynomial<E::Fr>,
        t_mid: &Polynomial<E::Fr>,
        t_hi: &Polynomial<E::Fr>,
        z_n: E::Fr,
        z_two_n: E::Fr,
    ) -> Polynomial<E::Fr> {
        let poly_zn = Polynomial::from_coefficients_slice(&[z_n]);
        let poly_z_two_n = Polynomial::from_coefficients_slice(&[z_two_n]);

        let zn_tmid_poly = t_mid * &poly_zn;
        let z_two_n_thi_poly = t_hi * &poly_z_two_n;

        &(&z_two_n_thi_poly + &zn_tmid_poly) + t_lo
    }

    fn compute_shifted_polynomial(
        &self,
        v_7: E::Fr,
        z_poly: &Polynomial<E::Fr>,
        z_eval: E::Fr,
        shifted_z: E::Fr,
    ) -> Polynomial<E::Fr> {
        let poly_z_eval = Polynomial::from_coefficients_slice(&[z_eval]);
        let poly_v_7 = Polynomial::from_coefficients_slice(&[v_7]);

        // Z(X) - z_eval
        let z_minus_z_eval = z_poly - &poly_z_eval;

        // v^7(Z(X) - z_eval)
        let t = &poly_v_7 * &z_minus_z_eval;

        // X - zw
        let divisor = Polynomial::from_coefficients_vec(vec![-shifted_z, E::Fr::one()]);

        &t / &divisor
    }

    // computes sum [ challenge[i] * (polynomial[i] - evaluations[i])]
    fn compute_challenge_poly_eval(
        &self,
        challenges: Vec<E::Fr>,
        polynomials: Vec<&Polynomial<E::Fr>>,
        evaluations: Vec<E::Fr>,
    ) -> Polynomial<E::Fr> {
        let sum = izip!(
            challenges.into_iter(),
            polynomials.into_iter(),
            evaluations.into_iter()
        )
        .map(|(v, poly, eval)| {
            let poly_eval = Polynomial::from_coefficients_slice(&[eval]);
            let poly_v = Polynomial::from_coefficients_slice(&[v]);

            let poly_minus_eval = poly - &poly_eval;

            &poly_v * &poly_minus_eval
        })
        .fold(Polynomial::zero(), |mut acc, val| {
            acc += &val;
            acc
        });

        sum
    }

    // Given P(X) and `z`. compute P(X) - P(z) / X - z
    fn compute_witness_polynomial(&self, p: &Polynomial<E::Fr>, z: E::Fr) -> Polynomial<E::Fr> {
        // evaluate polynomial at z
        let p_eval = p.evaluate(z);
        // convert value to a polynomial
        let poly_eval = Polynomial::from_coefficients_vec(vec![p_eval]);

        // Construct divisor for kate witness
        let divisor = Polynomial::from_coefficients_vec(vec![-z, E::Fr::one()]);

        // Compute witness polynomial
        let witness_polynomial = &(p - &poly_eval) / &divisor;

        witness_polynomial
    }
}