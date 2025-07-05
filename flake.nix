{
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs, ... }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
    in
    {
      packages.${system}.default = pkgs.rustPlatform.buildRustPackage {
        name = "buzkaaclicker-backend";
        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter =
            path: type:
            let
              baseName = baseNameOf path;
            in
            baseName != "target" && baseName != ".git";
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

          config = lib.mkIf config.services.buzkaaclicker-backend.enable {
            systemd.services.buzkaaclicker-backend = {
              description = "Buzkaa Clicker Backend";
              wantedBy = [ "multi-user.target" ];
              after = [ "network.target" ];
              serviceConfig = {
                ExecStart = "${self.packages.${pkgs.system}.default}/bin/bclicker-server";
                WorkingDirectory = config.users.users.buzkaaclicker-backend.home;
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
          };
        };
    };
}
