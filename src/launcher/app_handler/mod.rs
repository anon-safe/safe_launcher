// Copyright 2015 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under (1) the MaidSafe.net Commercial License,
// version 1.0 or later, or (2) The General Public License (GPL), version 3, depending on which
// licence you accepted on initial access to the Software (the "Licences").
//
// By contributing code to the SAFE Network Software, or to this project generally, you agree to be
// bound by the terms of the MaidSafe Contributor Agreement, version 1.0.  This, along with the
// Licenses can be found in the root directory of this project at LICENSE, COPYING and CONTRIBUTOR.
//
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.
//
// Please review the Licences for the specific language governing permissions and limitations
// relating to use of the SAFE Network Software.

pub mod events;

use std::io::{Read, Write};

mod misc;

const APP_HANDLER_THREAD_NAME: &'static str = "launcher.config";

pub struct AppHandler {
    client                 : ::std::sync::Arc<::std::sync::Mutex<::safe_core::client::Client>>,
    launcher_endpoint      : String,
    local_config_data      : ::std::collections::HashMap<::routing::NameType, String>,
    ipc_server_event_sender: ::launcher
                             ::ipc_server
                             ::EventSenderToServer<::launcher
                                                   ::ipc_server
                                                   ::events::ExternalEvent>,
}

impl AppHandler {
    pub fn new(client      : ::std::sync::Arc<::std::sync::Mutex<::safe_core::client::Client>>,
               event_sender: ::launcher::ipc_server
                                       ::EventSenderToServer<::launcher
                                                             ::ipc_server
                                                             ::events::ExternalEvent>) -> (::safe_core::utility::RAIIThreadJoiner,
                                                                                           ::std::sync::mpsc::Sender<events::AppHandlerEvent>) {
        let (event_tx, event_rx) = ::std::sync::mpsc::channel();

        let joiner = eval_result!(::std::thread::Builder::new().name(APP_HANDLER_THREAD_NAME.to_string())
                                                               .spawn(move || {
            let mut temp_dir_pathbuf = ::std::env::temp_dir();
            temp_dir_pathbuf.push(::config::LAUNCHER_LOCAL_CONFIG_FILE_NAME);

            let mut local_config_data = ::std::collections::HashMap::with_capacity(10);

            if let Ok(mut file) = ::std::fs::File::open(temp_dir_pathbuf) {
                let mut raw_disk_data = Vec::with_capacity(eval_result!(file.metadata()).len() as usize);
                if let Ok(_) = file.read_to_end(&mut raw_disk_data) {
                    if raw_disk_data.len() != 0 {
                        match eval_result!(client.lock()).hybrid_decrypt(&raw_disk_data, None) {
                            Ok(plain_text) => local_config_data = eval_result!(::safe_core::utility::deserialise(&plain_text)),
                            Err(err) => debug!("{:?} -> Local config file could not be read - either tampered or corrupted. Starting afresh...", err),
                        }
                    }
                }
            }

            let (tx, rx) = ::std::sync::mpsc::channel();
            eval_result!(event_sender.send(::launcher::ipc_server::events::ExternalEvent::GetListenerEndpoint(tx)));
            let launcher_endpoint = eval_result!(rx.recv());

            let app_handler = AppHandler {
                client                 : client,
                launcher_endpoint      : launcher_endpoint,
                local_config_data      : local_config_data,
                ipc_server_event_sender: event_sender,
            };

            AppHandler::run(app_handler, event_rx);

            debug!("Exiting thread {:?}", APP_HANDLER_THREAD_NAME);
        }));

        (::safe_core::utility::RAIIThreadJoiner::new(joiner), event_tx)
    }

    fn run(mut app_handler: AppHandler, event_rx: ::std::sync::mpsc::Receiver<events::AppHandlerEvent>) {
        for event in event_rx.iter() {
            match event {
                events::AppHandlerEvent::AddApp(app_detail)     => app_handler.on_add_app(app_detail),
                events::AppHandlerEvent::RemoveApp(app_id)      => app_handler.on_remove_app(app_id),
                events::AppHandlerEvent::ActivateApp(app_id)    => app_handler.on_activate_app(app_id),
                events::AppHandlerEvent::Terminate => break,
            }
        }
    }

    fn get_app_dir_name(app_name: &String,
                        directory_listing: &::safe_nfs::directory_listing::DirectoryListing) -> String {
        let mut index = 0u8;
        let mut dir_name = String::new();
        loop {
            dir_name = format!("{}-{}-Root-Dir", &app_name, index);
            match directory_listing.find_sub_directory(&dir_name) {
                Some(_) => index += 1,
                None => break,
            }
        };
        dir_name
    }

    fn tokenise_string(source: &str) -> Vec<String> {
        source .split(|element| element == '/')
               .filter(|token| token.len() != 0)
               .map(|token| token.to_string())
               .collect()
    }

    fn get_launcher_global_config(&self) -> Result<Vec<misc::LauncherConfiguration>, ::errors::LauncherError> {
        Ok(try!(self.get_launcher_global_config_and_dir()).0)
    }

    fn upsert_to_launcher_global_config(&self, config: misc::LauncherConfiguration) -> Result<(), ::errors::LauncherError> {
        let (mut global_configs, dir_listing) = try!(self.get_launcher_global_config_and_dir());

        // TODO(Spandan) Due to bug in the language, unable to use `if let Some() .. else` logic to
        // upsert to a vector. Once the bug is resolved
        // - https://github.com/rust-lang/rust/issues/28449
        // then modify the following to use it.
        if let Some(pos) = global_configs.iter().position(|existing_config| existing_config.app_id == config.app_id) {
            let existing_config = eval_option!(global_configs.get_mut(pos), "Logic Error - Report bug.");
            *existing_config = config;
        } else {
            global_configs.push(config);
        }

        let file = eval_option!(dir_listing.get_files()
                                           .iter()
                                           .find(|file| file.get_name() == ::config::LAUNCHER_GLOBAL_CONFIG_FILE_NAME),
                                "Logic Error - Launcher start-up should ensure the file must be present at this stage - Report bug.").clone();

        let file_helper = ::safe_nfs::helper::file_helper::FileHelper::new(self.client.clone());
        let mut writer = try!(file_helper.update_content(file, ::safe_nfs::helper::writer::Mode::Overwrite, dir_listing));
        writer.write(&try!(::safe_core::utility::serialise(&global_configs)), 0);
        let _ = try!(writer.close()); // TODO use result

        Ok(())
    }

    fn get_launcher_global_config_and_dir(&self) -> Result<(Vec<misc::LauncherConfiguration>,
                                                            ::safe_nfs::directory_listing::DirectoryListing),
                                                           ::errors::LauncherError> {
        let dir_helper = ::safe_nfs::helper::directory_helper::DirectoryHelper::new(self.client.clone());
        let dir_listing = try!(dir_helper.get_configuration_directory_listing(::config::LAUNCHER_GLOBAL_DIRECTORY_NAME.to_string()));

        let global_configs = {
            let file = eval_option!(dir_listing.get_files()
                                               .iter()
                                               .find(|file| file.get_name() == ::config::LAUNCHER_GLOBAL_CONFIG_FILE_NAME),
                                    "Logic Error - Launcher start-up should ensure the file must be present at this stage - Report bug.");

            let file_helper = ::safe_nfs::helper::file_helper::FileHelper::new(self.client.clone());
            let mut reader = file_helper.read(file);

            let size = reader.size();

            if size != 0 {
                try!(::safe_core::utility::deserialise(&try!(reader.read(0, size))))
            } else {
                Vec::new()
            }
        };

        Ok((global_configs, dir_listing))
    }

    //TODO instead of eval_result! retun error to asker
    fn on_add_app(&mut self, app_detail: Box<events::event_data::AppDetail>) {
        {
            let mut paths = self.local_config_data.values();
            if let Some(_) = paths.find(|stored_path| **stored_path == app_detail.absolute_path) {
                debug!("App already added");
                return
            }
        }

        let app_id = ::routing::NameType::new(eval_result!(::safe_core::utility::generate_random_array_u8_64()));

        let _ = self.local_config_data.insert(app_id, app_detail.absolute_path.clone());

        let mut tokens = AppHandler::tokenise_string(&app_detail.absolute_path);

        let mut app_name = eval_option!(tokens.pop(), ""); // TODO(Spandan) don't use eval_option here

        let dir_helper = ::safe_nfs::helper::directory_helper::DirectoryHelper::new(self.client.clone());
        let mut root_dir_listing = eval_result!(dir_helper.get_user_root_directory_listing());

        let app_dir_name = AppHandler::get_app_dir_name(&app_name, &root_dir_listing);
        let app_root_dir_key = match root_dir_listing.find_sub_directory(&app_dir_name).map(|dir| dir.clone()) {
            Some(app_dir) => app_dir.get_key().clone(),
            None => {
                eval_result!(dir_helper.create(app_dir_name,
                                               ::safe_nfs::UNVERSIONED_DIRECTORY_LISTING_TAG,
                                               Vec::new(),
                                               false,
                                               ::safe_nfs::AccessLevel::Private,
                                               Some(&mut root_dir_listing))).0.get_key().clone()
            },
        };

        let new_launcher_config = misc::LauncherConfiguration {
            app_id           : app_id,
            app_name         : app_name,
            reference_count  : 1,
            app_root_dir_key : app_root_dir_key,
            safe_drive_access: app_detail.safe_drive_access,
        };
        eval_result!(self.upsert_to_launcher_global_config(new_launcher_config));
    }

    fn on_activate_app(&mut self, app_id: Box<::routing::NameType>) {
        let global_configs = eval_result!(self.get_launcher_global_config());

        if let Some(app_info) = global_configs.iter().find(|config| config.app_id == *app_id) {
            if let Some(app_binary_path) = self.local_config_data.get(&app_info.app_id) {
                let str_nonce = eval_result!(::safe_core::utility::generate_random_string(::config::LAUNCHER_NONCE_LENGTH));
                let activation_detail = ::launcher::ipc_server::events::event_data::ActivationDetail {
                    nonce            : str_nonce.clone(),
                    app_id           : app_info.app_id.clone(),
                    app_root_dir_key : app_info.app_root_dir_key.clone(),
                    safe_drive_access: app_info.safe_drive_access,
                };

                eval_result!(self.ipc_server_event_sender.send(::launcher
                                                               ::ipc_server
                                                               ::events
                                                               ::ExternalEvent::AppActivated(Box::new(activation_detail))));

                let command_line_arg = format!("tcp:{}:{}", self.launcher_endpoint, str_nonce);

                let _app_process_handle = eval_result!(::std::process::Command::new(app_binary_path)
                                                                               .arg("--launcher")
                                                                               .arg(command_line_arg)
                                                                               .spawn());
            }
        }
    }

    // TODO(Krishna) Send terminate app event to IPC Server
    fn on_remove_app(&mut self, app_id: Box<::routing::NameType>) {
        // remove/update from launcher_configurations
        let config_file_name = ::config::LAUNCHER_GLOBAL_CONFIG_FILE_NAME.to_string();

        let dir_helper = ::safe_nfs::helper::directory_helper::DirectoryHelper::new(self.client.clone());
        let file_helper = ::safe_nfs::helper::file_helper::FileHelper::new(self.client.clone());
        let (mut launcher_configurations, dir_listing) = eval_result!(self.get_launcher_global_config_and_dir());

        let position = eval_option!(launcher_configurations.iter().position(|config| config.app_id == *app_id), "Logic Error - Report as bug.");
        let app_root_dir_key = launcher_configurations[position].app_root_dir_key.clone();
        let reference_count = launcher_configurations[position].reference_count;
        if reference_count == 1 {
            let _ = launcher_configurations.remove(position);
            let mut root_dir_listing = eval_result!(dir_helper.get_user_root_directory_listing());
            let app_root_dir_listing = eval_result!(dir_helper.get(&app_root_dir_key));
            let _ = eval_result!(dir_helper.delete(&mut root_dir_listing, app_root_dir_listing.get_metadata().get_name()));
        } else {
             let config = eval_option!(launcher_configurations.get_mut(position), "Logic Error - Report as bug.");
             config.reference_count -= 1;
        }
        let file = eval_option!(dir_listing.find_file(&config_file_name).map(|file| file.clone()), "Logic Error - Report as bug.");
        let mut writer = eval_result!(file_helper.update_content(file, ::safe_nfs::helper::writer::Mode::Overwrite, dir_listing));
        writer.write(&eval_result!(::safe_core::utility::serialise(&launcher_configurations)), 0);
        let _ = eval_result!(writer.close());

        // remove from local config
        let _ = self.local_config_data.remove(&app_id);
    }
}

impl Drop for AppHandler {
    fn drop(&mut self) {
        let mut temp_dir_pathbuf = ::std::env::temp_dir();
        temp_dir_pathbuf.push(::config::LAUNCHER_LOCAL_CONFIG_FILE_NAME);

        let mut file = eval_result!(::std::fs::File::create(temp_dir_pathbuf));
        let plain_text = eval_result!(::safe_core::utility::serialise(&self.local_config_data));
        let cipher_text = eval_result!(eval_result!(self.client.lock()).hybrid_encrypt(&plain_text, None));
        let _ = file.write_all(&cipher_text);
        eval_result!(file.sync_all());
    }
}