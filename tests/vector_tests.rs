#![allow(non_snake_case)]
mod common;

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        str::FromStr,
    };

    use secp256k1::{PublicKey, SecretKey};
    use silentpayments::{sending::{decode_scan_pubkey, generate_recipient_pubkeys}, receiving::SilentPayment };

    use crate::common::{
            structs::TestData,
            utils::{
                self, sender_calculate_shared_secret, decode_input_pub_keys, decode_outpoints,
                decode_outputs_to_check, decode_priv_keys, decode_recipients,
                get_a_sum_secret_keys, hash_outpoints, verify_and_calculate_signatures, get_A_sum_public_keys,
            },
        };

    const IS_TESTNET: bool = false;

    #[test]
    fn test_with_test_vectors() {
        let testdata = utils::read_file();

        for test in testdata {
            process_test_case(test);
        }
    }

    fn process_test_case(test_case: TestData) {
        let mut sending_outputs: HashSet<String> = HashSet::new();
        eprintln!("test.comment = {:?}", test_case.comment);
        for sendingtest in test_case.sending {
            let given = sendingtest.given;

            let expected = sendingtest.expected.outputs;

            let expected_output_addresses: HashSet<String> =
                expected.iter().map(|(x, _)| x.into()).collect();

            let input_priv_keys = decode_priv_keys(&given.input_priv_keys);

            let outpoints = decode_outpoints(&given.outpoints);

            let outpoints_hash = hash_outpoints(&outpoints);

            let silent_addresses = decode_recipients(&given.recipients);

            let a_sum = get_a_sum_secret_keys(&input_priv_keys);

            let mut ecdh_shared_secrets: HashMap<PublicKey, PublicKey> = HashMap::new();
            for addr in &silent_addresses {
                let B_scan = decode_scan_pubkey(addr.to_owned()).unwrap();
                let ecdh_shared_secret = sender_calculate_shared_secret(a_sum, B_scan, outpoints_hash);
                ecdh_shared_secrets.insert(B_scan, ecdh_shared_secret);
            }
            let outputs =
                generate_recipient_pubkeys(silent_addresses, ecdh_shared_secrets).unwrap();

            for output_pubkeys in &outputs {
                for pubkey in output_pubkeys.1 {
                    // TODO check if this is always true
                    sending_outputs.insert(hex::encode(pubkey.serialize()));
                }
            }

            assert_eq!(sending_outputs, expected_output_addresses);
        }

        for receivingtest in test_case.receiving {
            let given = receivingtest.given;
            let mut expected = receivingtest.expected;

            let receiving_outputs: HashSet<String> = given.outputs.iter().cloned().collect();

            // assert that the generated sending outputs are a subset
            // of the expected receiving outputs
            // i.e. all the generated outputs are present
            assert!(sending_outputs.is_subset(&receiving_outputs));

            let b_scan = SecretKey::from_str(&given.scan_priv_key).unwrap();
            let b_spend = SecretKey::from_str(&given.spend_priv_key).unwrap();

            let mut sp_receiver = SilentPayment::new(0, b_scan, b_spend, IS_TESTNET).unwrap();

            let labels = given.labels.iter().map(|l| l.1.to_owned()).collect();

            let receiving_addresses = sp_receiver.get_receiving_addresses(labels).unwrap();

            let set1: HashSet<_> = receiving_addresses.iter().map(|r| r.1).collect();
            let set2: HashSet<_> = expected.addresses.iter().collect();

            // check that the receiving addresses generated are equal
            // to the expected addresses
            assert_eq!(set1, set2);

            // can be even or odd !
            let outputs_to_check = decode_outputs_to_check(&given.outputs);

            let outpoints = decode_outpoints(&given.outpoints);

            let input_pub_keys = decode_input_pub_keys(&given.input_pub_keys);


            for (_, label) in &given.labels {
                sp_receiver.add_label(label.to_owned()).unwrap();
            }

            let A_sum = get_A_sum_public_keys(&input_pub_keys);
            let outpoints_hash = hash_outpoints(&outpoints);

            let ecdh_shared_secret = sp_receiver.calculate_shared_secret(A_sum, outpoints_hash).unwrap();

            let add_to_wallet = sp_receiver.scan_for_outputs(
                &ecdh_shared_secret,
                outputs_to_check,
            ).unwrap();

            let privkeys: Vec<SecretKey> = add_to_wallet.iter().flat_map(|(_, list)| {
                let mut ret: Vec<SecretKey> = vec![];
                for l in list {
                    ret.push(SecretKey::from_str(l).unwrap());
                }
                ret
            })
            .collect();

            let mut res = verify_and_calculate_signatures(privkeys, b_spend).unwrap();

            res.sort_by_key(|output| output.pub_key.clone());
            expected.outputs.sort_by_key(|output| output.pub_key.clone());

            assert_eq!(res, expected.outputs);
        }
    }
}
