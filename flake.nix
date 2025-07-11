{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs, ... }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
      binaries = builtins.fetchGit {
        url = "git@github.com-buzkaaclicker-bins:BuzkaaClicker/bins.git";
        rev = "ab668c1893b0c0586cfe1184b23bf09d7f82cb29";
      };
      frontend = builtins.fetchGit {
        url = "git@github.com-buzkaaclicker-frontend-og:BuzkaaClicker/frontend-og.git";
        rev = "58389653f1a364cf6038cbf2f3f6fbf84a56b890";
      };
    in
    {
      packages.${system}.default = pkgs.rustPlatform.buildRustPackage {
        name = "buzkaaclicker-backend";
        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter =
            path: _:
            let
              name = baseNameOf path;
            in
            name != "target" && name != ".git";
        };

        env = {
          BUILT_OVERRIDE_bclicker-server_GIT_VERSION = self.shortRev or "dirty";
        };

        cargoLock = {
          lockFile = ./Cargo.lock;
        };
      };

      nixosModules.default =
        {
          config,
          lib,
          pkgs,
          inputs,
          ...
        }:
        {
          options.services.buzkaaclicker-backend = {
            enable = lib.mkEnableOption "Buzkaa Clicker backend service";
            clickerVersion = lib.mkOption {
              type = lib.types.int;
              default = 16;
              description = "latest clicker version used by the updater";
            };
          };

          config = lib.mkIf config.services.buzkaaclicker-backend.enable (
            let
              homeDir = config.users.users.buzkaaclicker-backend.home;
            in
            {
              systemd.services.buzkaaclicker-backend = {
                description = "Buzkaa Clicker Backend";
                wantedBy = [ "multi-user.target" ];
                after = [ "network.target" ];
                serviceConfig = {
                  ExecStart = "${self.packages.${pkgs.system}.default}/bin/bclicker-server";
                  WorkingDirectory = homeDir;
                  Restart = "always";
                  User = "buzkaaclicker-backend";
                  Group = "buzkaaclicker-backend";
                };
                environment = {
                  BUZKAACLICKER_VERSION = builtins.toString config.services.buzkaaclicker-backend.clickerVersion;
                };
              };

              users.users.buzkaaclicker-backend = {
                isSystemUser = true;
                group = "buzkaaclicker-backend";
                createHome = true;
                home = "/home/buzkaaclicker-backend";
              };
              users.groups.buzkaaclicker-backend = { };

              systemd.tmpfiles.rules =
                let
                  # symlink all files, because i dont want to override this whole directory!
                  binRules =
                    [
                      "d ${homeDir}/filehost 0555 buzkaaclicker-backend buzkaaclicker-backend -"
                    ]
                    ++ (
                      let
                        filesDir = builtins.readDir (binaries);
                      in
                      filesDir
                      |> builtins.attrNames
                      |> builtins.filter (file: filesDir.${file} == "regular")
                      |> builtins.map (file: "L+ ${homeDir}/filehost/${file} - - - - ${binaries}/${file}")
                    );

                  staticRules = [
                    "L+ ${homeDir}/static - - - - ${frontend}"
                  ];
                in
                binRules ++ staticRules;
            }
          );
        };
    };
}
